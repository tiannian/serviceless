//! Hand-written output of a future `export!(S1, messages = [Add])` macro.
use std::{
    ffi::c_void,
    sync::{Arc, Condvar, Mutex},
};

use async_trait::async_trait;
use serviceless::{
    runtime::OneshotReceiver, service, Context, Handler, Message, Service, ServiceAddress,
};
use serviceless_dynamic::{Config, DynamicMessage, Failure, Success};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Add {
    pub amount: u64,
}

impl Message for Add {
    type Result = u64;
}
unsafe impl DynamicMessage for Add {
    type Result = u64;
    const NAME: &'static str = "Add";
}

struct S1 {
    count: u64,
}

#[service(name = "S1")]
impl Service for S1 {}

#[async_trait]
impl Handler<Add> for S1 {
    async fn handle(&mut self, message: Add, _ctx: &mut Context<Self>) -> u64 {
        self.count += message.amount;
        self.count
    }
}

struct Instance {
    address: ServiceAddress<S1>,
    task: tokio::task::JoinHandle<()>,
    runtime: Option<tokio::runtime::Runtime>,
    calls: Arc<(Mutex<usize>, Condvar)>,
}

#[no_mangle]
pub extern "C" fn __serviceless_S1_init(cfg: Config) -> *mut c_void {
    let result = std::panic::catch_unwind(|| {
        if cfg.isolated_runtime == 0 {
            return None;
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .ok()?;
        let service = S1 { count: 0 };
        let (address, run) = Context::new(&service).run(service);
        let task = runtime.spawn(run);
        Some(Box::into_raw(Box::new(Mutex::new(Instance {
            address,
            task,
            runtime: Some(runtime),
            calls: Arc::new((Mutex::new(0), Condvar::new())),
        }))) as *mut c_void)
    });
    result.ok().flatten().unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn __serviceless_S1_destroy(instance: *mut c_void) {
    if instance.is_null() {
        return;
    }
    let instance = Box::from_raw(instance as *mut Mutex<Instance>);
    let (calls, runtime) = {
        let mut guard = instance.lock().unwrap();
        guard.address.close_service();
        (guard.calls.clone(), guard.runtime.take())
    };
    let (count, done) = &*calls;
    let mut active = count.lock().unwrap();
    while *active != 0 {
        active = done.wait(active).unwrap();
    }
    drop(active);
    let guard = instance.lock().unwrap();
    guard.task.abort();
    drop(guard);
    if let Some(runtime) = runtime {
        runtime.shutdown_background();
    }
}

#[no_mangle]
pub unsafe extern "C" fn __serviceless_S1_send_Add(instance: *mut c_void, message: Add) -> i32 {
    if instance.is_null() {
        return -1;
    }
    let guard = match (*(instance as *const Mutex<Instance>)).lock() {
        Ok(g) => g,
        Err(_) => return -2,
    };
    match guard.address.send(message) {
        Ok(()) => 0,
        Err(_) => -3,
    }
}

#[no_mangle]
pub unsafe extern "C" fn __serviceless_S1_call_Add(
    instance: *mut c_void,
    message: Add,
    context: *mut c_void,
    success: Success<u64>,
    failed: Failure,
) -> i32 {
    if instance.is_null() || context.is_null() {
        return -1;
    }
    let (receiver, handle, calls) = {
        let guard = match (*(instance as *const Mutex<Instance>)).lock() {
            Ok(g) => g,
            Err(_) => return -2,
        };
        let receiver = match guard.address.call_raw(message) {
            Ok(r) => r,
            Err(_) => return -3,
        };
        let handle = guard.runtime.as_ref().unwrap().handle().clone();
        let calls = guard.calls.clone();
        *calls.0.lock().unwrap() += 1;
        (receiver, handle, calls)
    };
    let context_addr = context as usize;
    handle.spawn(async move {
        match receiver.recv().await {
            Ok(value) => success(context_addr as *mut c_void, &value),
            Err(_) => failed(context_addr as *mut c_void, -3),
        }
        let mut active = calls.0.lock().unwrap();
        *active -= 1;
        if *active == 0 {
            calls.1.notify_all();
        }
    });
    0
}

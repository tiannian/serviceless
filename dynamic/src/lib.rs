//! Load Serviceless services exported from a native dynamic library.
//!
//! The example in `examples/flatten-so` spells out the functions an export macro
//! would generate. Message and result layouts are a contract between host and plugin.
use std::{
    ffi::c_void,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use libloading::Library;
use tokio::sync::{oneshot, Notify};

/// Configuration passed to the plugin initializer.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// 1 creates one in the plugin. Shared runtime (0) is reserved for a future
    /// cross-library scheduler protocol.
    pub isolated_runtime: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            isolated_runtime: 1,
        }
    }
}

/// A message and its result that can be copied across the plugin boundary.
///
/// # Safety
/// Both types must have a C-compatible layout, contain no Rust-owned resources,
/// and have identical definitions in the host and plugin.
pub unsafe trait DynamicMessage: Copy + Send + 'static {
    type Result: Copy + Send + 'static;
    const NAME: &'static str;
}

pub type Success<R> = unsafe extern "C" fn(*mut c_void, *const R);
pub type Failure = unsafe extern "C" fn(*mut c_void, i32);
pub type InitFn = unsafe extern "C" fn(Config) -> *mut c_void;
pub type DestroyFn = unsafe extern "C" fn(*mut c_void);
pub type SendFn<M> = unsafe extern "C" fn(*mut c_void, M) -> i32;
pub type CallFn<M> = unsafe extern "C" fn(
    *mut c_void,
    M,
    *mut c_void,
    Success<<M as DynamicMessage>::Result>,
    Failure,
) -> i32;

#[derive(Debug)]
pub enum Error {
    Library(String),
    Symbol(String),
    InitFailed,
    Closed,
    Service(i32),
    CallbackDropped,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

pub struct SrvDynLoader {
    config: Config,
}

impl Default for SrvDynLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl SrvDynLoader {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }
    pub fn isolated_runtime(mut self, enabled: bool) -> Self {
        self.config.isolated_runtime = u8::from(enabled);
        self
    }

    /// Load a native library and initialize the named service.
    ///
    /// `path` may be a full file path or a directory containing the platform's
    /// native file name for `name` (for example `libS1.so` or `S1.dll`).
    ///
    /// # Safety
    /// The library must be trusted and implement the matching ABI and message layouts.
    pub unsafe fn load_lib(
        &self,
        name: &str,
        path: impl AsRef<Path>,
    ) -> Result<LoadedService, Error> {
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(Error::Symbol("invalid service name".into()));
        }
        let path = resolve_library_path(name, path.as_ref());
        let library = Library::new(&path).map_err(|e| Error::Library(e.to_string()))?;
        let init: InitFn = *library
            .get(format!("__serviceless_{name}_init\0").as_bytes())
            .map_err(|e| Error::Symbol(e.to_string()))?;
        let destroy: DestroyFn = *library
            .get(format!("__serviceless_{name}_destroy\0").as_bytes())
            .map_err(|e| Error::Symbol(e.to_string()))?;
        let instance = init(self.config);
        if instance.is_null() {
            return Err(Error::InitFailed);
        }
        Ok(LoadedService {
            inner: Arc::new(Inner {
                library,
                instance,
                destroy,
                name: name.to_owned(),
                state: Mutex::new(State {
                    closing: false,
                    pending: 0,
                }),
                notify: Notify::new(),
            }),
        })
    }
}

pub fn resolve_library_path(name: &str, path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(libloading::library_filename(name))
    } else {
        path.to_owned()
    }
}

struct State {
    closing: bool,
    pending: usize,
}
struct Inner {
    library: Library,
    instance: *mut c_void,
    destroy: DestroyFn,
    name: String,
    state: Mutex<State>,
    notify: Notify,
}

// The plugin protects its instance internally and the library stays loaded while Inner exists.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl Inner {
    fn finish(&self) {
        let mut state = self.state.lock().unwrap();
        state.pending -= 1;
        if state.pending == 0 {
            self.notify.notify_one();
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.instance) }
    }
}

pub struct LoadedService {
    inner: Arc<Inner>,
}

impl Drop for LoadedService {
    fn drop(&mut self) {
        // A callback may still be executing in the plugin. Without `close`,
        // retain the library rather than unload code under that callback.
        if self.inner.state.lock().unwrap().pending != 0 {
            std::mem::forget(self.inner.clone());
        }
    }
}

struct Callback<R> {
    sender: Option<oneshot::Sender<Result<R, Error>>>,
    inner: Arc<Inner>,
    _result: PhantomData<R>,
}

unsafe extern "C" fn success<R: Copy + Send + 'static>(ctx: *mut c_void, value: *const R) {
    let mut callback = Box::from_raw(ctx as *mut Callback<R>);
    let result = if value.is_null() {
        Err(Error::CallbackDropped)
    } else {
        Ok(*value)
    };
    if let Some(sender) = callback.sender.take() {
        let _ = sender.send(result);
    }
    callback.inner.finish();
}

unsafe extern "C" fn failure<R: Copy + Send + 'static>(ctx: *mut c_void, code: i32) {
    let mut callback = Box::from_raw(ctx as *mut Callback<R>);
    if let Some(sender) = callback.sender.take() {
        let _ = sender.send(Err(Error::Service(code)));
    }
    callback.inner.finish();
}

impl LoadedService {
    pub fn send<M: DynamicMessage>(&self, message: M) -> Result<(), Error> {
        let state = self.inner.state.lock().unwrap();
        if state.closing {
            return Err(Error::Closed);
        }
        let symbol = format!("__serviceless_{}_send_{}\0", self.inner.name, M::NAME);
        let send: SendFn<M> = unsafe {
            *self
                .inner
                .library
                .get(symbol.as_bytes())
                .map_err(|e| Error::Symbol(e.to_string()))?
        };
        let code = unsafe { send(self.inner.instance, message) };
        if code == 0 {
            Ok(())
        } else {
            Err(Error::Service(code))
        }
    }

    pub async fn call<M: DynamicMessage>(&self, message: M) -> Result<M::Result, Error> {
        let (receiver, call, context) = {
            let mut state = self.inner.state.lock().unwrap();
            if state.closing {
                return Err(Error::Closed);
            }
            let symbol = format!("__serviceless_{}_call_{}\0", self.inner.name, M::NAME);
            let call: CallFn<M> = unsafe {
                *self
                    .inner
                    .library
                    .get(symbol.as_bytes())
                    .map_err(|e| Error::Symbol(e.to_string()))?
            };
            let (sender, receiver) = oneshot::channel();
            state.pending += 1;
            let callback = Box::new(Callback {
                sender: Some(sender),
                inner: self.inner.clone(),
                _result: PhantomData,
            });
            let context = Box::into_raw(callback) as *mut c_void;
            (receiver, call, context)
        };
        let code = unsafe {
            call(
                self.inner.instance,
                message,
                context,
                success::<M::Result>,
                failure::<M::Result>,
            )
        };
        if code != 0 {
            // A nonzero return means the plugin did not take ownership of context.
            unsafe {
                drop(Box::from_raw(context as *mut Callback<M::Result>));
            }
            self.inner.finish();
        }
        if code != 0 {
            return Err(Error::Service(code));
        }
        receiver.await.map_err(|_| Error::CallbackDropped)?
    }

    /// Reject new calls and wait for all accepted callbacks before unloading.
    pub async fn close(self) {
        loop {
            let notified = self.inner.notify.notified();
            {
                let mut state = self.inner.state.lock().unwrap();
                state.closing = true;
                if state.pending == 0 {
                    break;
                }
            }
            notified.await;
        }
    }
}

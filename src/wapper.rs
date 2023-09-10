use std::io;

use crate::{AsyncStepService, Service, StepError};

macro_rules! define_async_step_service_wapper {
    ($name:ident, $($field:ident => $type:ident)*) => {
        #[derive(getset::Getters, getset::MutGetters)]
        pub struct $name<$( $type, )*> {
            $(
                #[getset(get = "pub", get_mut = "pub")]
                $field: $type,
            )*
            exit_flag: bool,
        }

        impl<$( $type, )*> $name<$( $type, )*> {
            pub fn new($( $field: $type, )*) -> Self {
                Self {
                    $(
                        $field,
                    )*
                    exit_flag: true,
                }
            }
        }


        impl<$( $type, )*> Service for $name<$( $type, )*>
        where
            $(
                $type: AsyncStepService,
            )*
        {
            type Error = io::Error;

            fn start(&mut self) -> Result<(), Self::Error> {
                let rt = tokio::runtime::Runtime::new()?;
                tokio_scoped::scoped(rt.handle()).scope(|scope| {

                    $(
                        scope.spawn(async {
                            while self.exit_flag {
                                if let Err(e) = self.$field.step().await {
                                    log::error!("Async Step Service Error: {}", e);

                                    if e.is_exit() {
                                        break;
                                    }
                                }
                            }
                        });
                    )*

                });

                Ok(())
            }

            fn stop(&mut self) -> Result<(), Self::Error> {
                self.exit_flag = false;
                Ok(())
            }
        }
    };
}

define_async_step_service_wapper!(AsyncStepServiceWapper1, service0 => S0);
define_async_step_service_wapper!(AsyncStepServiceWapper2, service0 => S0 service1 => S1);
define_async_step_service_wapper!(AsyncStepServiceWapper3, service0 => S0 service1 => S1 service2 => S2);

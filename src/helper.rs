macro_rules! try_log {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                error!("Error: {}", err);
                return;
            }
        }
    };
}
pub(crate) use try_log;
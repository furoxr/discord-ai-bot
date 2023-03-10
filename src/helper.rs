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

// Extract value from qdrant::Value, return Err if value is not present or has different Kind
macro_rules! try_match {
    ($expr:expr, $key:expr, $kind:tt) => {
        match $expr.get($key) {
            Some(value) => {
                if let Some(Kind::$kind(value)) = value.kind.clone() {
                    value
                } else {
                    return Err(anyhow!("'{}' has different Kind", $key))
                }
            }
            None => {
                return Err(anyhow!("'{}' is not present", $key))
            }
        }
    };
}

pub(crate) use try_match;
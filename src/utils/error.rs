use crate::utils;
use std::fmt::{Display, Formatter};

/// 业务错误+错误的调用链路
#[macro_export]
macro_rules! biz_err {
    ($msg:expr) => {{
        use anyhow::Error;
        Error::msg($msg).context(format!("{}:{}", file!(), line!()))
    }};
    ($fmt:expr $(, $arg:tt)*) => {{
        use anyhow::Error;
        let msg = format!($fmt $(, $arg)*);
        Error::msg(msg).context(format!("{}:{}", file!(), line!()))
    }};
}

/// 追加调用链路
#[macro_export]
macro_rules! trace_err {
    ($e:expr) => {{
        $e.context(format!("{}:{}", file!(), line!()))
    }};
}

pub struct Error {
    pub traces: Vec<String>,
    pub cause: String,
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        let mut traces = Vec::new();
        traces.push(value.to_string());

        let mut source = value.source();
        let mut cause = String::new();
        while let Some(s) = source {
            traces.push(s.to_string());
            cause = s.to_string();
            source = s.source();
        }

        if traces.len() > 0 {
            traces.remove(traces.len() - 1);
        }

        return Error { traces, cause };
    }
}

impl From<&anyhow::Error> for Error {
    fn from(value: &anyhow::Error) -> Self {
        let mut traces = Vec::new();
        traces.push(value.to_string());

        let mut source = value.source();
        let mut cause = String::new();
        while let Some(s) = source {
            traces.push(s.to_string());
            cause = s.to_string();
            source = s.source();
        }

        if traces.len() > 0 {
            traces.remove(traces.len() - 1);
        }

        return Error { traces, cause };
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // 先显示 cause
        write!(f, "{}", self.cause)?;

        // 如果有 traces，按行显示每个 trace
        if !self.traces.is_empty() {
            write!(
                f,
                ", traces: {}",
                utils::json::to_string(&self.traces).unwrap_or("[]".to_string())
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error() {
        let err = biz_err!("hello world: {}", "leo");
        // let err1 = trace_err!(err);
        // let err2 = trace_err!(err1);
        // let err3 = trace_err!(err2);

        let e: Error = err.into();
        println!("{}", e);
    }
}

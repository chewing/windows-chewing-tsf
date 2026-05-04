use std::{error::Error, fmt::Display};

pub trait ResultExt<T, E>: Sized {
    fn boxed<'a>(self) -> Result<T, Box<dyn Error + Send + Sync + 'a>>
    where
        E: Error + Send + Sync + 'a;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn boxed<'a>(self) -> Result<T, Box<dyn Error + Send + Sync + 'a>>
    where
        E: Error + Send + Sync + 'a,
    {
        self.map_err(|e| Box::new(e) as _)
    }
}

pub struct Report<'a, T>(pub &'a T);

impl<T> Display for Report<'_, T>
where
    T: Error,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Error: {}", self.0)?;
        if self.0.source().is_some() {
            write!(f, "Caused by:")?;
            let mut index = 0;
            let mut parent = self.0.source();
            while let Some(src) = parent {
                write!(f, "\n    {index}: {}", src)?;
                index += 1;
                parent = src.source();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Display;

    use super::Report;

    #[derive(Debug)]
    struct Error(&'static str, Box<dyn std::error::Error>);
    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self.1.as_ref())
        }
    }
    impl Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    #[test]
    fn test_report() {
        let errors = Error(
            "Failed to do something",
            Error(
                "Failed due to internal error",
                std::io::Error::other("Failed to perform IO").into(),
            )
            .into(),
        );
        assert_eq!(
            Report(&errors).to_string(),
            "Error: Failed to do something\nCaused by:\n    0: Failed due to internal error\n    1: Failed to perform IO"
        );
    }
}

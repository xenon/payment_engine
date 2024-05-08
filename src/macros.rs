// Macro to only do eprintln! if the feature flag 'printerrors' is set
// This prevents the program from being slowed by a file with a bunch of errors
#[cfg(feature = "printerrors")]
macro_rules! eprintln_featureflag {
    ($($arg:tt)*) => ({
        eprintln!($($arg)*);
    })
}

// This macro just suppresses the unused variable warnings when not using the eprintln
#[cfg(not(feature = "printerrors"))]
macro_rules! eprintln_featureflag {
    ($fmtstr:expr $(, $arg:expr)*) => {{
        $(let _ = $arg;)*
    }};
}

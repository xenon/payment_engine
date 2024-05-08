// Macro to only do eprintln! if the feature flag 'printerrors' is set
// This prevents the program from being slowed by a file with a bunch of errors
macro_rules! eprintln_featureflag {
    ($($arg:tt)*) => ({
        #[cfg(feature = "printerrors")]
        eprintln!($($arg)*);
    })
}

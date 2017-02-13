use time::now_utc;
use hdrsample;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct Histogram {
    pub timestamp: (i64, i32), // Seconds, Nanoseconds (UTC)
    pub len: u64,
    pub low: u64,
    pub high: u64,
    pub sigfig: u32,
    pub count: u64,
    pub last: u64,
    pub min: u64,
    pub max: u64,
    pub mean: u32, // (Conversion from float)
    pub stdev: u32, // (Conversion from float)
    pub percentiles: Vec<Bucket>
}

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct Bucket {
    pub percentile: u32, // float percentile multiplied by 10^sigfig
    pub value: u64
}

impl From<hdrsample::Histogram<u64>> for Histogram {
    fn from(h: hdrsample::Histogram<u64>) -> Self {
        let now = now_utc().to_timespec();
        Histogram {
            timestamp: (now.sec, now.nsec),
            len: h.len() as u64,
            low: h.low() as u64,
            high: h.high(),
            sigfig: h.sigfig(),
            count: h.count(),
            last: h.last() as u64,
            min: h.min(),
            max: h.max(),
            mean: float_to_int(h.mean(), h.sigfig()),
            stdev: float_to_int(h.stdev(), h.sigfig()),
            percentiles: h.iter_percentiles(1).map(|iv| Bucket {
                percentile: float_to_int(iv.percentile(), h.sigfig()),
                value: iv.value()
            }).collect()
        }
    }
}

// Convert a float to int without losing fractional part.
//
// float `f` multiplied by 10^sigfig and then cast to a u32
//
// Note that this only works on small numbers. It will provide
// incorrect results for anything larger than a u32.
// For the usecase of this module, this limitation is fine.
fn float_to_int(f: f64, sigfig: u32) -> u32 {
    (f * (10.0_f64).powf(sigfig as f64)) as u32
}

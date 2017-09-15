use std::fmt::{self, Debug, Formatter};
use hdrsample;
use hdrsample::serialization::Deserializer as hdrsampleDeserializer;
use hdrsample::serialization::V2Serializer;
use serde::ser::{self, Serialize, Serializer};
use serde::de::{self, Deserialize, Deserializer};
use serde_bytes::{Bytes, ByteBuf};
use msgpack;


#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds
}

/// A histogram that can be serialized via Serde
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram(hdrsample::Histogram<u64>);

impl Histogram {
    pub fn new() -> Histogram {
        Histogram(hdrsample::Histogram::<u64>::new(3).unwrap())
    }
}

/// A typed histogram specifies a time unit
#[derive(Clone, PartialEq, Serialize)]
pub struct TypedHistogram {
    pub unit: TimeUnit,
    pub histogram: Histogram
}

impl TypedHistogram {
    pub fn new(unit: TimeUnit) -> TypedHistogram {
        TypedHistogram {
            unit: unit,
            histogram: Histogram::new()
        }
    }
}

impl Debug for TypedHistogram {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Histogram ({:?})", self.unit)
    }
}

impl Serialize for Histogram {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        // Serialize the histogram using it's native V2 serialization and then using serde
        let mut buf = Vec::new();
        V2Serializer::new().serialize(&self.0, &mut buf)
                           .map_err(|e| ser::Error::custom(format!("{:?}", e)))?;

        // This is much more efficient than just serializing each byte individually via
        // serialize_bytes. See https://github.com/serde-rs/serde/issues/518
        let buf = Bytes::new(&buf);
        buf.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Histogram {
    fn deserialize<D>(deserializer: D) -> Result<Histogram, D::Error>
        where D: Deserializer<'de>
    {
        let buf = ByteBuf::deserialize(deserializer)?;
        let histogram = hdrsampleDeserializer::new()
            .deserialize(&mut &buf[..])
            .map_err(|e| de::Error::custom(format!("{:?}", e)))?;
        Ok(Histogram(histogram))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_serialization() {
        let mut hist = Histogram::new();
        for _ in 0..10 {
            hist.0.record(1).unwrap();
        }
        hist.0.record(10).unwrap();
        let num_samples = hist.0.count();
        let _99th = hist.0.value_at_percentile(99.9);
        let _50th = hist.0.value_at_percentile(50.0);

        let mut serialized = Vec::new();
        hist.serialize(&mut msgpack::Serializer::new(&mut serialized)).unwrap();

        let mut deserializer = msgpack::Deserializer::new(&serialized[..]);
        let deserialized = Deserialize::deserialize(&mut deserializer).unwrap();

        assert_eq!(hist, deserialized);
        assert_eq!(num_samples, deserialized.0.count());
        assert_eq!(_99th, deserialized.0.value_at_percentile(99.9));
        assert_eq!(_50th, deserialized.0.value_at_percentile(50.0));
    }
}

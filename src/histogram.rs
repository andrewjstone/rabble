use std::fmt::{self, Debug, Formatter};
use hdrsample;
use hdrsample::serialization::Deserializer as hdrsampleDeserializer;
use hdrsample::serialization::V2Serializer;
use serde::ser::{self, Serialize, Serializer};
use serde::de::{self, Deserialize, Deserializer};
use serde_bytes::Bytes;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds
}

/// A histogram that can be serialized via Serde
#[derive(Clone, PartialEq)]
pub struct Histogram(hdrsample::Histogram<u64>);

/// A typed histogram specifies a time unit
#[derive(Clone, PartialEq, Serialize)]
pub struct TypedHistogram {
    pub unit: TimeUnit,
    pub histogram: Histogram
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
        let buf = Bytes::deserialize(deserializer)?;
        let histogram = hdrsampleDeserializer::new()
            .deserialize(&mut &*buf)
            .map_err(|e| de::Error::custom(format!("{:?}", e)))?;
        Ok(Histogram(histogram))
    }
}

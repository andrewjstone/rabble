use serde::{Serialize, Deserialize};
use std::fmt::Debug;
use histogram::Histogram;

// A container type for status information for a given component
pub trait Metrics<'de>: Serialize + Deserialize<'de> + Debug + Clone {
    fn data(&self) -> Vec<(String, Metric)>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Metric {
    Gauge(i64),
    Counter(u64),
    Histogram(Histogram)
}

/// Generate a struct: `$struct_name` from a set of metrics
///
/// Generate the impl containing the constructor, `$struct_name::new()`
/// Generate `impl Metrics for $struct_name` constructing the Metric
/// variants returned from `$struct_name::data` based on the type of the struct fields.
macro_rules! metrics {
    ($struct_name:ident {
        $( $field:ident: $ty:ident ),+
    }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $struct_name {
            $( pub $field: $ty ),+
        }

        impl $struct_name {
            pub fn new() -> $struct_name {
                $struct_name {
                    $( $field: $ty::default() ),+
                }
            }
        }

        impl<'de> Metrics<'de> for $struct_name {
            fn data(&self) -> Vec<(String, Metric)> {
                vec![
                    $( (stringify!($field).into(), type_to_metric!($ty)(self.$field.clone())) ),+
                    ]
            }
        }
    }
}

macro_rules! type_to_metric {
    (i64) => { Metric::Gauge };
    (u64) => { Metric::Counter };
    (Histogram) => { Metric::Histogram };
}

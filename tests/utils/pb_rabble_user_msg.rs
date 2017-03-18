// This file is generated. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]

use protobuf::Message as Message_imported_for_functions;
use protobuf::ProtobufEnum as ProtobufEnum_imported_for_functions;

#[derive(Clone,Default)]
pub struct PbRabbleUserMsg {
    // message fields
    op: ::std::option::Option<u64>,
    op_complete: ::std::option::Option<bool>,
    get_history: ::std::option::Option<bool>,
    history: ::protobuf::SingularPtrField<History>,
    // special fields
    unknown_fields: ::protobuf::UnknownFields,
    cached_size: ::std::cell::Cell<u32>,
}

// see codegen.rs for the explanation why impl Sync explicitly
unsafe impl ::std::marker::Sync for PbRabbleUserMsg {}

impl PbRabbleUserMsg {
    pub fn new() -> PbRabbleUserMsg {
        ::std::default::Default::default()
    }

    pub fn default_instance() -> &'static PbRabbleUserMsg {
        static mut instance: ::protobuf::lazy::Lazy<PbRabbleUserMsg> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const PbRabbleUserMsg,
        };
        unsafe {
            instance.get(|| {
                PbRabbleUserMsg {
                    op: ::std::option::Option::None,
                    op_complete: ::std::option::Option::None,
                    get_history: ::std::option::Option::None,
                    history: ::protobuf::SingularPtrField::none(),
                    unknown_fields: ::protobuf::UnknownFields::new(),
                    cached_size: ::std::cell::Cell::new(0),
                }
            })
        }
    }

    // optional uint64 op = 1;

    pub fn clear_op(&mut self) {
        self.op = ::std::option::Option::None;
    }

    pub fn has_op(&self) -> bool {
        self.op.is_some()
    }

    // Param is passed by value, moved
    pub fn set_op(&mut self, v: u64) {
        self.op = ::std::option::Option::Some(v);
    }

    pub fn get_op(&self) -> u64 {
        self.op.unwrap_or(0)
    }

    // optional bool op_complete = 2;

    pub fn clear_op_complete(&mut self) {
        self.op_complete = ::std::option::Option::None;
    }

    pub fn has_op_complete(&self) -> bool {
        self.op_complete.is_some()
    }

    // Param is passed by value, moved
    pub fn set_op_complete(&mut self, v: bool) {
        self.op_complete = ::std::option::Option::Some(v);
    }

    pub fn get_op_complete(&self) -> bool {
        self.op_complete.unwrap_or(false)
    }

    // optional bool get_history = 3;

    pub fn clear_get_history(&mut self) {
        self.get_history = ::std::option::Option::None;
    }

    pub fn has_get_history(&self) -> bool {
        self.get_history.is_some()
    }

    // Param is passed by value, moved
    pub fn set_get_history(&mut self, v: bool) {
        self.get_history = ::std::option::Option::Some(v);
    }

    pub fn get_get_history(&self) -> bool {
        self.get_history.unwrap_or(false)
    }

    // optional .History history = 4;

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn has_history(&self) -> bool {
        self.history.is_some()
    }

    // Param is passed by value, moved
    pub fn set_history(&mut self, v: History) {
        self.history = ::protobuf::SingularPtrField::some(v);
    }

    // Mutable pointer to the field.
    // If field is not initialized, it is initialized with default value first.
    pub fn mut_history(&mut self) -> &mut History {
        if self.history.is_none() {
            self.history.set_default();
        };
        self.history.as_mut().unwrap()
    }

    // Take field
    pub fn take_history(&mut self) -> History {
        self.history.take().unwrap_or_else(|| History::new())
    }

    pub fn get_history(&self) -> &History {
        self.history.as_ref().unwrap_or_else(|| History::default_instance())
    }
}

impl ::protobuf::Message for PbRabbleUserMsg {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !try!(is.eof()) {
            let (field_number, wire_type) = try!(is.read_tag_unpack());
            match field_number {
                1 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    };
                    let tmp = try!(is.read_uint64());
                    self.op = ::std::option::Option::Some(tmp);
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    };
                    let tmp = try!(is.read_bool());
                    self.op_complete = ::std::option::Option::Some(tmp);
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    };
                    let tmp = try!(is.read_bool());
                    self.get_history = ::std::option::Option::Some(tmp);
                },
                4 => {
                    try!(::protobuf::rt::read_singular_message_into(wire_type, is, &mut self.history));
                },
                _ => {
                    try!(::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields()));
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        for value in &self.op {
            my_size += ::protobuf::rt::value_size(1, *value, ::protobuf::wire_format::WireTypeVarint);
        };
        if self.op_complete.is_some() {
            my_size += 2;
        };
        if self.get_history.is_some() {
            my_size += 2;
        };
        for value in &self.history {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        };
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream) -> ::protobuf::ProtobufResult<()> {
        if let Some(v) = self.op {
            try!(os.write_uint64(1, v));
        };
        if let Some(v) = self.op_complete {
            try!(os.write_bool(2, v));
        };
        if let Some(v) = self.get_history {
            try!(os.write_bool(3, v));
        };
        if let Some(v) = self.history.as_ref() {
            try!(os.write_tag(4, ::protobuf::wire_format::WireTypeLengthDelimited));
            try!(os.write_raw_varint32(v.get_cached_size()));
            try!(v.write_to_with_cached_sizes(os));
        };
        try!(os.write_unknown_fields(self.get_unknown_fields()));
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn type_id(&self) -> ::std::any::TypeId {
        ::std::any::TypeId::of::<PbRabbleUserMsg>()
    }

    fn as_any(&self) -> &::std::any::Any {
        self as &::std::any::Any
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        ::protobuf::MessageStatic::descriptor_static(None::<Self>)
    }
}

impl ::protobuf::MessageStatic for PbRabbleUserMsg {
    fn new() -> PbRabbleUserMsg {
        PbRabbleUserMsg::new()
    }

    fn descriptor_static(_: ::std::option::Option<PbRabbleUserMsg>) -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_singular_u64_accessor(
                    "op",
                    PbRabbleUserMsg::has_op,
                    PbRabbleUserMsg::get_op,
                ));
                fields.push(::protobuf::reflect::accessor::make_singular_bool_accessor(
                    "op_complete",
                    PbRabbleUserMsg::has_op_complete,
                    PbRabbleUserMsg::get_op_complete,
                ));
                fields.push(::protobuf::reflect::accessor::make_singular_bool_accessor(
                    "get_history",
                    PbRabbleUserMsg::has_get_history,
                    PbRabbleUserMsg::get_get_history,
                ));
                fields.push(::protobuf::reflect::accessor::make_singular_message_accessor(
                    "history",
                    PbRabbleUserMsg::has_history,
                    PbRabbleUserMsg::get_history,
                ));
                ::protobuf::reflect::MessageDescriptor::new::<PbRabbleUserMsg>(
                    "PbRabbleUserMsg",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }
}

impl ::protobuf::Clear for PbRabbleUserMsg {
    fn clear(&mut self) {
        self.clear_op();
        self.clear_op_complete();
        self.clear_get_history();
        self.clear_history();
        self.unknown_fields.clear();
    }
}

impl ::std::cmp::PartialEq for PbRabbleUserMsg {
    fn eq(&self, other: &PbRabbleUserMsg) -> bool {
        self.op == other.op &&
        self.op_complete == other.op_complete &&
        self.get_history == other.get_history &&
        self.history == other.history &&
        self.unknown_fields == other.unknown_fields
    }
}

impl ::std::fmt::Debug for PbRabbleUserMsg {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

#[derive(Clone,Default)]
pub struct History {
    // message fields
    history: ::std::vec::Vec<u64>,
    // special fields
    unknown_fields: ::protobuf::UnknownFields,
    cached_size: ::std::cell::Cell<u32>,
}

// see codegen.rs for the explanation why impl Sync explicitly
unsafe impl ::std::marker::Sync for History {}

impl History {
    pub fn new() -> History {
        ::std::default::Default::default()
    }

    pub fn default_instance() -> &'static History {
        static mut instance: ::protobuf::lazy::Lazy<History> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const History,
        };
        unsafe {
            instance.get(|| {
                History {
                    history: ::std::vec::Vec::new(),
                    unknown_fields: ::protobuf::UnknownFields::new(),
                    cached_size: ::std::cell::Cell::new(0),
                }
            })
        }
    }

    // repeated uint64 history = 1;

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    // Param is passed by value, moved
    pub fn set_history(&mut self, v: ::std::vec::Vec<u64>) {
        self.history = v;
    }

    // Mutable pointer to the field.
    pub fn mut_history(&mut self) -> &mut ::std::vec::Vec<u64> {
        &mut self.history
    }

    // Take field
    pub fn take_history(&mut self) -> ::std::vec::Vec<u64> {
        ::std::mem::replace(&mut self.history, ::std::vec::Vec::new())
    }

    pub fn get_history(&self) -> &[u64] {
        &self.history
    }
}

impl ::protobuf::Message for History {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !try!(is.eof()) {
            let (field_number, wire_type) = try!(is.read_tag_unpack());
            match field_number {
                1 => {
                    try!(::protobuf::rt::read_repeated_uint64_into(wire_type, is, &mut self.history));
                },
                _ => {
                    try!(::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields()));
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        for value in &self.history {
            my_size += ::protobuf::rt::value_size(1, *value, ::protobuf::wire_format::WireTypeVarint);
        };
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream) -> ::protobuf::ProtobufResult<()> {
        for v in &self.history {
            try!(os.write_uint64(1, *v));
        };
        try!(os.write_unknown_fields(self.get_unknown_fields()));
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn type_id(&self) -> ::std::any::TypeId {
        ::std::any::TypeId::of::<History>()
    }

    fn as_any(&self) -> &::std::any::Any {
        self as &::std::any::Any
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        ::protobuf::MessageStatic::descriptor_static(None::<Self>)
    }
}

impl ::protobuf::MessageStatic for History {
    fn new() -> History {
        History::new()
    }

    fn descriptor_static(_: ::std::option::Option<History>) -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_repeated_u64_accessor(
                    "history",
                    History::get_history,
                ));
                ::protobuf::reflect::MessageDescriptor::new::<History>(
                    "History",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }
}

impl ::protobuf::Clear for History {
    fn clear(&mut self) {
        self.clear_history();
        self.unknown_fields.clear();
    }
}

impl ::std::cmp::PartialEq for History {
    fn eq(&self, other: &History) -> bool {
        self.history == other.history &&
        self.unknown_fields == other.unknown_fields
    }
}

impl ::std::fmt::Debug for History {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

static file_descriptor_proto_data: &'static [u8] = &[
    0x0a, 0x18, 0x70, 0x62, 0x5f, 0x72, 0x61, 0x62, 0x62, 0x6c, 0x65, 0x5f, 0x75, 0x73, 0x65, 0x72,
    0x5f, 0x6d, 0x73, 0x67, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x22, 0x87, 0x01, 0x0a, 0x0f, 0x50,
    0x62, 0x52, 0x61, 0x62, 0x62, 0x6c, 0x65, 0x55, 0x73, 0x65, 0x72, 0x4d, 0x73, 0x67, 0x12, 0x0e,
    0x0a, 0x02, 0x6f, 0x70, 0x18, 0x01, 0x20, 0x01, 0x28, 0x04, 0x52, 0x02, 0x6f, 0x70, 0x12, 0x1f,
    0x0a, 0x0b, 0x6f, 0x70, 0x5f, 0x63, 0x6f, 0x6d, 0x70, 0x6c, 0x65, 0x74, 0x65, 0x18, 0x02, 0x20,
    0x01, 0x28, 0x08, 0x52, 0x0a, 0x6f, 0x70, 0x43, 0x6f, 0x6d, 0x70, 0x6c, 0x65, 0x74, 0x65, 0x12,
    0x1f, 0x0a, 0x0b, 0x67, 0x65, 0x74, 0x5f, 0x68, 0x69, 0x73, 0x74, 0x6f, 0x72, 0x79, 0x18, 0x03,
    0x20, 0x01, 0x28, 0x08, 0x52, 0x0a, 0x67, 0x65, 0x74, 0x48, 0x69, 0x73, 0x74, 0x6f, 0x72, 0x79,
    0x12, 0x22, 0x0a, 0x07, 0x68, 0x69, 0x73, 0x74, 0x6f, 0x72, 0x79, 0x18, 0x04, 0x20, 0x01, 0x28,
    0x0b, 0x32, 0x08, 0x2e, 0x48, 0x69, 0x73, 0x74, 0x6f, 0x72, 0x79, 0x52, 0x07, 0x68, 0x69, 0x73,
    0x74, 0x6f, 0x72, 0x79, 0x22, 0x23, 0x0a, 0x07, 0x48, 0x69, 0x73, 0x74, 0x6f, 0x72, 0x79, 0x12,
    0x18, 0x0a, 0x07, 0x68, 0x69, 0x73, 0x74, 0x6f, 0x72, 0x79, 0x18, 0x01, 0x20, 0x03, 0x28, 0x04,
    0x52, 0x07, 0x68, 0x69, 0x73, 0x74, 0x6f, 0x72, 0x79, 0x4a, 0x9b, 0x03, 0x0a, 0x06, 0x12, 0x04,
    0x00, 0x00, 0x0b, 0x01, 0x0a, 0x08, 0x0a, 0x01, 0x0c, 0x12, 0x03, 0x00, 0x00, 0x12, 0x0a, 0x0a,
    0x0a, 0x02, 0x04, 0x00, 0x12, 0x04, 0x02, 0x00, 0x07, 0x01, 0x0a, 0x0a, 0x0a, 0x03, 0x04, 0x00,
    0x01, 0x12, 0x03, 0x02, 0x08, 0x17, 0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x00, 0x02, 0x00, 0x12, 0x03,
    0x03, 0x02, 0x19, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x00, 0x04, 0x12, 0x03, 0x03, 0x02,
    0x0a, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x00, 0x05, 0x12, 0x03, 0x03, 0x0b, 0x11, 0x0a,
    0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x00, 0x01, 0x12, 0x03, 0x03, 0x12, 0x14, 0x0a, 0x0c, 0x0a,
    0x05, 0x04, 0x00, 0x02, 0x00, 0x03, 0x12, 0x03, 0x03, 0x17, 0x18, 0x0a, 0x0b, 0x0a, 0x04, 0x04,
    0x00, 0x02, 0x01, 0x12, 0x03, 0x04, 0x02, 0x20, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x01,
    0x04, 0x12, 0x03, 0x04, 0x02, 0x0a, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x01, 0x05, 0x12,
    0x03, 0x04, 0x0b, 0x0f, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x01, 0x01, 0x12, 0x03, 0x04,
    0x10, 0x1b, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x01, 0x03, 0x12, 0x03, 0x04, 0x1e, 0x1f,
    0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x00, 0x02, 0x02, 0x12, 0x03, 0x05, 0x02, 0x20, 0x0a, 0x0c, 0x0a,
    0x05, 0x04, 0x00, 0x02, 0x02, 0x04, 0x12, 0x03, 0x05, 0x02, 0x0a, 0x0a, 0x0c, 0x0a, 0x05, 0x04,
    0x00, 0x02, 0x02, 0x05, 0x12, 0x03, 0x05, 0x0b, 0x0f, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02,
    0x02, 0x01, 0x12, 0x03, 0x05, 0x10, 0x1b, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x02, 0x03,
    0x12, 0x03, 0x05, 0x1e, 0x1f, 0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x00, 0x02, 0x03, 0x12, 0x03, 0x06,
    0x02, 0x1f, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x03, 0x04, 0x12, 0x03, 0x06, 0x02, 0x0a,
    0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x03, 0x06, 0x12, 0x03, 0x06, 0x0b, 0x12, 0x0a, 0x0c,
    0x0a, 0x05, 0x04, 0x00, 0x02, 0x03, 0x01, 0x12, 0x03, 0x06, 0x13, 0x1a, 0x0a, 0x0c, 0x0a, 0x05,
    0x04, 0x00, 0x02, 0x03, 0x03, 0x12, 0x03, 0x06, 0x1d, 0x1e, 0x0a, 0x0a, 0x0a, 0x02, 0x04, 0x01,
    0x12, 0x04, 0x09, 0x00, 0x0b, 0x01, 0x0a, 0x0a, 0x0a, 0x03, 0x04, 0x01, 0x01, 0x12, 0x03, 0x09,
    0x08, 0x0f, 0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x01, 0x02, 0x00, 0x12, 0x03, 0x0a, 0x02, 0x1e, 0x0a,
    0x0c, 0x0a, 0x05, 0x04, 0x01, 0x02, 0x00, 0x04, 0x12, 0x03, 0x0a, 0x02, 0x0a, 0x0a, 0x0c, 0x0a,
    0x05, 0x04, 0x01, 0x02, 0x00, 0x05, 0x12, 0x03, 0x0a, 0x0b, 0x11, 0x0a, 0x0c, 0x0a, 0x05, 0x04,
    0x01, 0x02, 0x00, 0x01, 0x12, 0x03, 0x0a, 0x12, 0x19, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x01, 0x02,
    0x00, 0x03, 0x12, 0x03, 0x0a, 0x1c, 0x1d,
];

static mut file_descriptor_proto_lazy: ::protobuf::lazy::Lazy<::protobuf::descriptor::FileDescriptorProto> = ::protobuf::lazy::Lazy {
    lock: ::protobuf::lazy::ONCE_INIT,
    ptr: 0 as *const ::protobuf::descriptor::FileDescriptorProto,
};

fn parse_descriptor_proto() -> ::protobuf::descriptor::FileDescriptorProto {
    ::protobuf::parse_from_bytes(file_descriptor_proto_data).unwrap()
}

pub fn file_descriptor_proto() -> &'static ::protobuf::descriptor::FileDescriptorProto {
    unsafe {
        file_descriptor_proto_lazy.get(|| {
            parse_descriptor_proto()
        })
    }
}

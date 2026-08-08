use std::collections::HashMap;
use std::ffi::CString;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CoreBluetoothError;
use crate::private::encode_json;
use crate::uuid::BluetoothUuid;

#[derive(Debug, Clone, Default)]
#[must_use]
/// Advertisement payload decoded from, or encoded for, `CoreBluetooth` advertisement dictionaries such as `CBAdvertisementDataLocalNameKey`.
pub struct AdvertisementData {
    raw: Value,
    local_name: Option<String>,
    tx_power_level: Option<i32>,
    service_uuids: Vec<BluetoothUuid>,
    service_data: HashMap<String, Vec<u8>>,
    manufacturer_data: Option<Vec<u8>>,
    overflow_service_uuids: Vec<BluetoothUuid>,
    is_connectable: Option<bool>,
    solicited_service_uuids: Vec<BluetoothUuid>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct AdvertisementPayload {
    pub local_name: Option<String>,
    pub service_uuids: Vec<String>,
}

impl AdvertisementData {
    /// Creates an empty advertisement dictionary matching the `CBAdvertisementData` payload shape.
    pub fn new() -> Self {
        Self {
            raw: Value::Object(serde_json::Map::default()),
            ..Self::default()
        }
    }

    /// Sets the `CBAdvertisementDataLocalNameKey` value.
    pub fn with_local_name(mut self, local_name: impl Into<String>) -> Self {
        self.local_name = Some(local_name.into());
        self
    }

    /// Adds a service UUID to the `CBAdvertisementDataServiceUUIDsKey` list.
    pub fn with_service_uuid(mut self, uuid: BluetoothUuid) -> Self {
        self.service_uuids.push(uuid);
        self
    }

    /// Decodes advertisement data from the JSON dictionary emitted by the `CoreBluetooth` bridge.
    pub fn from_json_value(raw: Value) -> Result<Self, CoreBluetoothError> {
        let object = raw.as_object().cloned().unwrap_or_default();
        let local_name = object
            .get("kCBAdvDataLocalName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let tx_power_level = object
            .get("kCBAdvDataTxPowerLevel")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok());
        let service_uuids = parse_uuid_array(object.get("kCBAdvDataServiceUUIDs"))?;
        let manufacturer_data = object
            .get("kCBAdvDataManufacturerData")
            .and_then(bytes_from_value);
        let overflow_service_uuids =
            parse_uuid_array(object.get("kCBAdvDataOverflowServiceUUIDs"))?;
        let is_connectable = object
            .get("kCBAdvDataIsConnectable")
            .and_then(Value::as_bool);
        let solicited_service_uuids =
            parse_uuid_array(object.get("kCBAdvDataSolicitedServiceUUIDs"))?;
        let service_data = parse_service_data(object.get("kCBAdvDataServiceData"));

        Ok(Self {
            raw,
            local_name,
            tx_power_level,
            service_uuids,
            service_data,
            manufacturer_data,
            overflow_service_uuids,
            is_connectable,
            solicited_service_uuids,
        })
    }

    /// Returns the `CBAdvertisementDataLocalNameKey` value.
    pub fn local_name(&self) -> Option<&str> {
        self.local_name.as_deref()
    }

    /// Returns the `CBAdvertisementDataTxPowerLevelKey` value.
    pub fn tx_power_level(&self) -> Option<i32> {
        self.tx_power_level
    }

    /// Returns the `CBAdvertisementDataServiceUUIDsKey` values.
    pub fn service_uuids(&self) -> &[BluetoothUuid] {
        &self.service_uuids
    }

    /// Returns the `CBAdvertisementDataServiceDataKey` payload.
    pub fn service_data(&self) -> &HashMap<String, Vec<u8>> {
        &self.service_data
    }

    /// Returns the `CBAdvertisementDataManufacturerDataKey` payload.
    pub fn manufacturer_data(&self) -> Option<&[u8]> {
        self.manufacturer_data.as_deref()
    }

    /// Returns the `CBAdvertisementDataOverflowServiceUUIDsKey` values.
    pub fn overflow_service_uuids(&self) -> &[BluetoothUuid] {
        &self.overflow_service_uuids
    }

    /// Returns the `CBAdvertisementDataIsConnectable` flag.
    pub fn is_connectable(&self) -> Option<bool> {
        self.is_connectable
    }

    /// Returns the `CBAdvertisementDataSolicitedServiceUUIDsKey` values.
    pub fn solicited_service_uuids(&self) -> &[BluetoothUuid] {
        &self.solicited_service_uuids
    }

    /// Returns the raw JSON dictionary produced by the `CoreBluetooth` bridge.
    pub fn raw(&self) -> &Value {
        &self.raw
    }

    pub(crate) fn encode_for_advertising(&self) -> Result<CString, CoreBluetoothError> {
        encode_json(&AdvertisementPayload {
            local_name: self.local_name.clone(),
            service_uuids: self
                .service_uuids
                .iter()
                .map(BluetoothUuid::uuid_string)
                .collect(),
        })
    }
}

fn parse_uuid_array(value: Option<&Value>) -> Result<Vec<BluetoothUuid>, CoreBluetoothError> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(BluetoothUuid::from_string)
                .collect()
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn parse_service_data(value: Option<&Value>) -> HashMap<String, Vec<u8>> {
    value
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    bytes_from_value(value).map(|bytes| (key.clone(), bytes))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn bytes_from_value(value: &Value) -> Option<Vec<u8>> {
    value.as_array().map(|items| {
        items
            .iter()
            .filter_map(Value::as_u64)
            .filter_map(|value| u8::try_from(value).ok())
            .collect()
    })
}

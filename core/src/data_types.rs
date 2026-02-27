use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::data_container::DataKind;

#[derive(Eq, Hash, PartialEq, EnumIter, Clone, Debug)]
#[allow(non_camel_case_types, dead_code)]
pub enum DataTypes {
    error_type = 0,
    error_protocol = 1,
    accepted_ids = 2,
    uuid = 3,
    register_id = 4,

    link = 5,

    settings = 6,
    settings_name = 7,
    chat_partner_id = 8,
    chat_partner_name = 9,
    iota_id = 10,
    user_id = 11,
    user_ids = 12,
    iota_ids = 13,
    user_state = 14,
    user_states = 15,
    user_pings = 16,
    call_state = 17,
    screen_share = 18,
    private_key_hash = 19,
    accepted = 20,
    accepted_profiles = 21,
    denied_profiles = 22,
    content = 23,
    messages = 24,
    notifications = 25,
    send_time = 26,
    get_time = 27,
    get_variant = 28,
    shared_secret_own = 29,
    shared_secret_other = 30,
    shared_secret_sign = 31,
    shared_secret = 32,
    call_id = 33,
    call_token = 34,
    untill = 35,
    enabled = 36,
    start_date = 37,
    end_date = 38,
    receiver_id = 39,
    sender_id = 40,
    signature = 41,
    signed = 42,
    message = 43,
    message_state = 44,
    last_ping = 45,
    ping_iota = 46,
    ping_clients = 47,
    matches = 48,
    omikron = 49,
    offset = 50,
    amount = 51,
    position = 52,
    name = 53,
    path = 54,
    codec = 55,
    function = 56,
    payload = 57,
    result = 58,
    interactables = 59,
    want_to_watch = 60,
    watcher = 61,
    created_at = 62,
    username = 63,
    display = 64,
    avatar = 65,
    about = 66,
    status = 67,
    public_key = 68,
    sub_level = 69,
    sub_end = 70,
    community_address = 71,
    challenge = 72,
    community_title = 73,
    communities = 74,
    rho_connections = 75,
    user = 76,
    online_status = 77,
    omikron_id = 78,
    omikron_connections = 79,
    reset_token = 80,
    new_token = 81,
}
impl DataTypes {
    pub fn expected_kind(&self) -> DataKind {
        match self {
            DataTypes::user_id
            | DataTypes::sender_id
            | DataTypes::receiver_id
            | DataTypes::call_id
            | DataTypes::amount
            | DataTypes::position
            | DataTypes::offset
            | DataTypes::sub_level => DataKind::Number,

            DataTypes::username
            | DataTypes::display
            | DataTypes::avatar
            | DataTypes::about
            | DataTypes::public_key
            | DataTypes::message
            | DataTypes::content
            | DataTypes::path
            | DataTypes::codec
            | DataTypes::function
            | DataTypes::challenge => DataKind::Str,

            DataTypes::messages
            | DataTypes::communities
            | DataTypes::rho_connections
            | DataTypes::matches => DataKind::Array(Box::new(DataKind::Container)),

            DataTypes::notifications
            | DataTypes::iota_ids
            | DataTypes::user_ids
            | DataTypes::omikron_connections => DataKind::Array(Box::new(DataKind::Number)),

            DataTypes::settings | DataTypes::user | DataTypes::payload | DataTypes::result => {
                DataKind::Container
            }

            DataTypes::enabled | DataTypes::signed | DataTypes::accepted => DataKind::Bool,

            _ => DataKind::Null,
        }
    }

    pub fn as_number(&self) -> u8 {
        DataTypes::iter().position(|v| v == *self).unwrap_or(0) as u8
    }
    pub fn from_number(n: u8) -> DataTypes {
        DataTypes::iter()
            .nth(n as usize)
            .unwrap_or(DataTypes::error_protocol)
    }
    pub fn parse(p0: String) -> DataTypes {
        for datatype in DataTypes::iter() {
            if datatype.to_string().to_lowercase().replace('_', "")
                == p0.to_lowercase().replace('_', "")
            {
                return datatype;
            }
        }
        DataTypes::error_type
    }
    pub fn to_string(&self) -> String {
        return format!("{:?}", self);
    }
}

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::communication::data_container::DataKind;

#[derive(Eq, Hash, PartialEq, EnumIter, Clone, Debug)]
#[allow(non_camel_case_types, dead_code)]
pub enum DataTypes {
    error_type,
    error_protocol,
    accepted_ids,
    uuid,
    register_id,

    link,

    settings,
    settings_name,
    chat_partner_id,
    chat_partner_name,
    iota_id,
    user_id,
    user_ids,
    iota_ids,
    user_state,
    user_states,
    user_pings,
    call_state,
    screen_share,
    private_key_hash,
    accepted,
    accepted_profiles,
    denied_profiles,
    content,
    messages,
    notifications,
    send_time,
    get_time,
    get_variant,
    shared_secret_own,
    shared_secret_other,
    shared_secret_sign,
    shared_secret,
    call_id,
    call_token,
    untill,
    enabled,
    start_date,
    end_date,
    receiver_id,
    sender_id,
    signature,
    signed,
    message,
    message_state,
    last_ping,
    ping_iota,
    ping_clients,
    matches,
    omikron,
    offset,
    amount,
    position,
    name,
    path,
    codec,
    function,
    payload,
    result,
    interactables,
    want_to_watch,
    watcher,
    created_at,
    username,
    display,
    avatar,
    about,
    status,
    public_key,
    sub_level,
    sub_end,
    community_address,
    challenge,
    community_title,
    communities,
    rho_connections,
    user,
    online_status,
    omikron_id,
    omikron_connections,
    reset_token,
    new_token,
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

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(PartialEq, Eq, Clone, EnumIter, Debug)]
#[allow(non_camel_case_types, dead_code)]
pub enum CommunicationType {
    error = 0,
    error_protocol = 1,
    error_anonymous = 2,
    error_internal = 3,
    error_invalid_data = 4,
    error_invalid_user_id = 5,
    error_invalid_omikron_id = 6,
    error_not_found = 7,
    error_not_authenticated = 8,
    error_no_iota = 9,
    error_invalid_challenge = 10,
    error_invalid_secret = 11,
    error_invalid_private_key = 12,
    error_invalid_public_key = 13,
    error_no_user_id = 14,
    error_no_call_id = 15,
    error_invalid_call_id = 16,
    success = 17,

    shorten_link = 18,

    settings_save = 19,
    settings_load = 20,
    settings_list = 21,
    message = 22,
    message_state = 23,
    message_send = 24,
    message_live = 25,
    message_other_iota = 26,
    message_chunk = 27,
    messages_get = 28,

    push_notification = 29,
    read_notification = 30,
    get_notifications = 31,

    change_confirm = 32,
    confirm_receive = 33,
    confirm_read = 34,
    get_chats = 35,
    get_states = 36,
    add_community = 37,
    remove_community = 38,
    get_communities = 39,
    challenge = 40,
    challenge_response = 41,
    register = 42,
    register_response = 43,
    identification = 44,
    identification_response = 45,
    register_iota = 46,
    register_iota_success = 47,
    ping = 48,
    pong = 49,
    add_conversation = 50,
    send_chat = 51,
    client_changed = 52,
    client_connected = 53,
    client_disconnected = 54,
    client_closed = 55,
    public_key = 56,
    private_key = 57,
    webrtc_sdp = 58,
    webrtc_ice = 59,
    start_stream = 60,
    end_stream = 61,
    watch_stream = 62,
    call_token = 63,
    call_invite = 64,
    call_disconnect_user = 65,
    call_timeout_user = 66,
    call_set_anonymous_joining = 67,
    end_call = 68,
    function = 69,
    update = 70,
    create_user = 71,
    rho_update = 72,

    user_connected = 73,
    user_disconnected = 74,
    iota_connected = 75,
    iota_disconnected = 76,
    sync_client_iota_status = 77,

    get_user_data = 78,
    get_iota_data = 79,
    iota_user_data = 80,

    change_user_data = 81,
    change_iota_data = 82,

    get_register = 83,
    complete_register_user = 84,
    complete_register_iota = 85,
    delete_user = 86,
    delete_iota = 87,

    start_register = 88,
    complete_register = 89,
}
impl CommunicationType {
    pub fn as_number(&self) -> u8 {
        CommunicationType::iter()
            .position(|v| v == *self)
            .unwrap_or(0) as u8
    }
    pub fn from_number(n: u8) -> CommunicationType {
        CommunicationType::iter()
            .nth(n as usize)
            .unwrap_or(CommunicationType::error_protocol)
    }

    pub fn parse(p0: String) -> CommunicationType {
        for datatype in CommunicationType::iter() {
            if datatype.to_string().to_lowercase().replace('_', "")
                == p0.to_lowercase().replace('_', "")
            {
                return datatype;
            }
        }
        CommunicationType::error_protocol
    }
    pub fn to_string(&self) -> String {
        return format!("{:?}", self);
    }
}

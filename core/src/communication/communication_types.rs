use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(PartialEq, Clone, EnumIter, Debug)]
#[allow(non_camel_case_types, dead_code)]
pub enum CommunicationType {
    error,
    error_protocol,
    error_anonymous,
    error_internal,
    error_invalid_data,
    error_invalid_user_id,
    error_invalid_omikron_id,
    error_not_found,
    error_not_authenticated,
    error_no_iota,
    error_invalid_challenge,
    error_invalid_secret,
    error_invalid_private_key,
    error_invalid_public_key,
    error_no_user_id,
    error_no_call_id,
    error_invalid_call_id,
    success,

    shorten_link,

    settings_save,
    settings_load,
    settings_list,
    message,
    message_state,
    message_send,
    message_live,
    message_other_iota,
    message_chunk,
    messages_get,

    push_notification,
    read_notification,
    get_notifications,

    change_confirm,
    confirm_receive,
    confirm_read,
    get_chats,
    get_states,
    add_community,
    remove_community,
    get_communities,
    challenge,
    challenge_response,
    register,
    register_response,
    identification,
    identification_response,
    register_iota,
    register_iota_success,
    ping,
    pong,
    add_conversation,
    send_chat,
    client_changed,
    client_connected,
    client_disconnected,
    client_closed,
    public_key,
    private_key,
    webrtc_sdp,
    webrtc_ice,
    start_stream,
    end_stream,
    watch_stream,
    call_token,
    call_invite,
    call_disconnect_user,
    call_timeout_user,
    call_set_anonymous_joining,
    end_call,
    function,
    update,
    create_user,
    rho_update,

    user_connected,
    user_disconnected,
    iota_connected,
    iota_disconnected,
    sync_client_iota_status,

    get_user_data,
    get_iota_data,
    iota_user_data,

    change_user_data,
    change_iota_data,

    get_register,
    complete_register_user,
    complete_register_iota,
    delete_user,
    delete_iota,

    start_register,
    complete_register,
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

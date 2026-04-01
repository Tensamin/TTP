use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Eq, Hash, PartialEq, EnumIter, Clone, Debug, PartialOrd, Ord)]
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

    call_invited = 82,
    call_members = 83,
    calls = 84,

    timeout = 85,
    has_admin = 86,

    last_message_at = 87,
    height = 88,

    sent_by_self = 89,
}
impl DataTypes {
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

#[cfg(test)]
mod tests {
    use crate::CommunicationType;

    use super::*;

    #[test]
    fn test_as_number_and_from_number_roundtrip() {
        for datatype in DataTypes::iter() {
            let number = datatype.as_number();
            let reconstructed = DataTypes::from_number(number);
            assert_eq!(datatype, reconstructed);
        }
    }

    #[test]
    fn test_from_number_invalid_defaults_to_error_protocol() {
        let invalid = 255;
        let res = DataTypes::from_number(invalid);
        assert_eq!(res, DataTypes::error_protocol);
    }

    #[test]
    fn test_parse_exact_match() {
        let parsed = DataTypes::parse("user_id".to_string());
        assert_eq!(parsed, DataTypes::user_id);
    }

    #[test]
    fn test_parse_case_insensitive() {
        let parsed = DataTypes::parse("UsEr_Id".to_string());
        assert_eq!(parsed, DataTypes::user_id);
    }

    #[test]
    fn test_parse_ignores_underscores() {
        let parsed = DataTypes::parse("userid".to_string());
        assert_eq!(parsed, DataTypes::user_id);
    }

    #[test]
    fn test_parse_invalid_defaults_to_error_type() {
        let parsed = DataTypes::parse("not_existing_type".to_string());
        assert_eq!(parsed, DataTypes::error_type);
    }

    #[test]
    fn test_to_string_matches_debug() {
        let datatype = DataTypes::call_id;
        assert_eq!(datatype.to_string(), "call_id");
    }

    #[test]
    fn test_as_number_consistency() {
        assert_eq!(DataTypes::error_type.as_number(), 0);
        assert_eq!(DataTypes::error_protocol.as_number(), 1);
        assert_eq!(DataTypes::accepted_ids.as_number(), 2);
    }

    #[test]
    fn test_from_number_boundary() {
        assert_eq!(DataTypes::from_number(0), DataTypes::error_type);
        let max_index = DataTypes::iter().count() as u8 - 1;
        assert_ne!(DataTypes::from_number(max_index), DataTypes::error_protocol);

        assert_eq!(
            DataTypes::from_number(max_index + 1),
            DataTypes::error_protocol
        );
    }

    #[test]
    fn test_parse_variations() {
        assert_eq!(DataTypes::parse("user_id".to_string()), DataTypes::user_id);
        assert_eq!(DataTypes::parse("userId".to_string()), DataTypes::user_id);
        assert_eq!(DataTypes::parse("User_Id".to_string()), DataTypes::user_id);
        assert_eq!(DataTypes::parse("USERID".to_string()), DataTypes::user_id);
    }

    #[test]
    fn test_to_string_format() {
        let s = DataTypes::call_id.to_string();
        assert!(!s.contains('"'));
        assert!(!s.contains(' '));
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn test_clone_equality() {
        let original = DataTypes::user_id;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_ord_sorting() {
        let mut types: Vec<DataTypes> = vec![
            DataTypes::user_id,
            DataTypes::error_type,
            DataTypes::username,
        ];
        types.sort();

        assert_eq!(types[0], DataTypes::error_type);
        assert_eq!(types[1], DataTypes::user_id);
        assert_eq!(types[2], DataTypes::username);
    }

    #[test]
    fn test_communication_type_as_number_matches_position() {
        for (idx, comm_type) in CommunicationType::iter().enumerate() {
            assert_eq!(comm_type.as_number() as usize, idx);
        }
    }

    #[test]
    fn test_communication_type_roundtrip_all() {
        for comm_type in CommunicationType::iter() {
            let num = comm_type.as_number();
            let reconstructed = CommunicationType::from_number(num);
            assert_eq!(comm_type, reconstructed);
        }
    }

    #[test]
    fn test_communication_type_parse_all() {
        for comm_type in CommunicationType::iter() {
            let name = comm_type.to_string();
            let parsed = CommunicationType::parse(name);
            assert_eq!(comm_type, parsed);
        }
    }

    #[test]
    fn test_communication_type_parse_invalid() {
        assert_eq!(
            CommunicationType::parse("nonexistent".to_string()),
            CommunicationType::error_protocol
        );
        assert_eq!(
            CommunicationType::parse("".to_string()),
            CommunicationType::error_protocol
        );
    }

    #[test]
    fn test_data_types_count() {
        let count = DataTypes::iter().count();
        assert!(count > 80, "Expected many DataTypes, got {}", count);
    }

    #[test]
    fn test_communication_types_count() {
        let count = CommunicationType::iter().count();
        assert!(
            count > 85,
            "Expected many CommunicationTypes, got {}",
            count
        );
    }

    #[test]
    fn test_error_types_grouped() {
        assert_eq!(DataTypes::error_type.as_number(), 0);
        assert_eq!(DataTypes::error_protocol.as_number(), 1);

        assert_eq!(CommunicationType::error.as_number(), 0);
        assert_eq!(CommunicationType::error_protocol.as_number(), 1);
    }
}

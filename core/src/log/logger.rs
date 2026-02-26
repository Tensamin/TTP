pub fn format_cv(cv: &CommunicationValue) -> String {
    let mut output = String::new();

    output.push_str("CommunicationValue {\n");

    output.push_str(&format!("  type: {:?}\n", cv.get_type()));

    if cv.get_id() != 0 {
        output.push_str(&format!("  id: {}\n", cv.get_id()));
    }

    if cv.get_sender() != 0 {
        output.push_str(&format!("  sender: {}\n", cv.get_sender()));
    }

    if cv.get_receiver() != 0 {
        output.push_str(&format!("  receiver: {}\n", cv.get_receiver()));
    }

    output.push_str("  data: {\n");

    for (key, value) in &cv.data {
        output.push_str(&format!(
            "    {}: {}\n",
            key.to_string(),
            format_data_value(value, 2)
        ));
    }

    output.push_str("  }\n");
    output.push('}');

    output
}
fn format_data_value(value: &DataValue, indent: usize) -> String {
    let indent_str = "  ".repeat(indent);

    match value {
        DataValue::Number(n) => n.to_string(),

        DataValue::Str(s) => format!("\"{}\"", s),

        DataValue::Bool(b) => b.to_string(),

        DataValue::BoolTrue => "true".to_string(),

        DataValue::BoolFalse => "false".to_string(),

        DataValue::Null => "null".to_string(),

        DataValue::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_string();
            }

            let mut out = String::from("[\n");

            for item in arr {
                out.push_str(&indent_str);
                out.push_str("  ");
                out.push_str(&format_data_value(item, indent + 1));
                out.push('\n');
            }

            out.push_str(&"  ".repeat(indent - 1));
            out.push(']');
            out
        }

        DataValue::Container(entries) => {
            if entries.is_empty() {
                return "{}".to_string();
            }

            let mut out = String::from("{\n");

            for (key, val) in entries {
                out.push_str(&indent_str);
                out.push_str(&format!(
                    "  {}: {}\n",
                    key.to_string(),
                    format_data_value(val, indent + 1)
                ));
            }

            out.push_str(&"  ".repeat(indent - 1));
            out.push('}');
            out
        }
    }
}

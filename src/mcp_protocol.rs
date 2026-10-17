use serde_json::{json, Value};
pub const VERSION: &str = "2025-06-18";
pub const MAX_FRAME: usize = 2_000_000;

pub fn tools() -> Value {
    let profile = json!({"type":"string","minLength":1,"maxLength":128});
    let position = json!({"type":"integer","minimum":1,"maximum":15});
    let mut step_schema = json!({"oneOf":[
        {"type":"object","properties":{"type":{"const":"delay"},"ms":{"type":"integer","minimum":0,"maximum":3600000}},"required":["type","ms"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"const":"hotkey"},"keys":{"type":"string"}},"required":["type","keys"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"const":"text"},"text":{"type":"string","maxLength":100000}},"required":["type","text"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"const":"launch"},"program":{"type":"string"},"args":{"type":"array","items":{"type":"string"},"maxItems":128},"focus_if_running":{"type":"boolean"}},"required":["type","program","args"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"const":"url"},"url":{"type":"string"}},"required":["type","url"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"const":"wait_window"},"title":{"type":"string"},"timeout_ms":{"type":"integer","minimum":100,"maximum":3600000},"focus":{"type":"boolean"}},"required":["type","title","timeout_ms","focus"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"const":"media"},"command":{"enum":["play_pause","next","previous","stop","mute","volume_up","volume_down"]}},"required":["type","command"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"const":"profile"},"name":profile},"required":["type","name"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"enum":["page","folder"]},"id":{"type":"string","maxLength":160}},"required":["type","id"],"additionalProperties":false},
        {"type":"object","properties":{"type":{"enum":["back","page_next","page_previous"]}},"required":["type"],"additionalProperties":false}
    ]});
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"close_window"}},"required":["type"],"additionalProperties":false}));
    step_schema["oneOf"].as_array_mut().unwrap().extend([
        json!({"type":"object","properties":{"type":{"const":"mouse_click"},"button":{"enum":["left","right","middle"]},"double":{"type":"boolean"},"x":{"type":"integer","minimum":-32768,"maximum":32767},"y":{"type":"integer","minimum":-32768,"maximum":32767},"relative_to":{"enum":["screen","window","cursor"]}},"required":["type"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"mouse_move"},"x":{"type":"integer","minimum":-32768,"maximum":32767},"y":{"type":"integer","minimum":-32768,"maximum":32767},"relative_to":{"enum":["screen","window","cursor"]},"relative":{"type":"boolean"}},"required":["type"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"mouse_scroll"},"amount":{"type":"integer","minimum":-100,"maximum":100},"horizontal":{"type":"boolean"}},"required":["type"],"additionalProperties":false}),
    ]);
    step_schema["oneOf"].as_array_mut().unwrap().extend([
        json!({"type":"object","properties":{"type":{"const":"play_sound"},"path":{"type":"string","minLength":1,"maxLength":1024},"volume":{"type":"integer","minimum":0,"maximum":100}},"required":["type","path","volume"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"stop_sounds"}},"required":["type"],"additionalProperties":false}),
    ]);
    step_schema["oneOf"].as_array_mut().unwrap().extend([
        json!({"type":"object","properties":{"type":{"const":"d6_brightness"},"mode":{"enum":["set","up","down"]},"value":{"type":"integer","minimum":0,"maximum":100}},"required":["type","mode","value"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"d6_sleep"}},"required":["type"],"additionalProperties":false}),
    ]);
    step_schema["oneOf"].as_array_mut().unwrap().extend([
        json!({"type":"object","properties":{"type":{"const":"udp_send"},"host":{"type":"string","minLength":1,"maxLength":253},"port":{"type":"integer","minimum":1,"maximum":65535},"text":{"type":"string","maxLength":65507}},"required":["type","host","port","text"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"http_request"},"method":{"enum":["GET","POST","PUT"]},"url":{"type":"string","maxLength":2048,"pattern":"^http://"},"body":{"type":"string","maxLength":65536},"content_type":{"type":"string","maxLength":128}},"required":["type","method","url","body","content_type"],"additionalProperties":false}),
    ]);
    let nested = json!({"type":"array","items":{"$ref":"#/$defs/step"},"maxItems":256});
    step_schema["oneOf"].as_array_mut().unwrap().extend([
        json!({"type":"object","properties":{"type":{"const":"device_volume"},"input":{"type":"boolean","default":false},"device":{"type":"string","minLength":1,"maxLength":512,"default":"default"},"action":{"enum":["up","down","set","mute","unmute","toggle_mute"],"default":"up"},"step":{"type":"integer","minimum":1,"maximum":100,"default":5},"level":{"type":"integer","minimum":0,"maximum":100,"default":50}},"required":["type"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"app_volume"},"target":{"type":"string","minLength":1,"maxLength":260,"description":"foreground ou nom d’exécutable, ex. chrome.exe"},"action":{"enum":["up","down","set","mute","unmute","toggle_mute"]},"step":{"type":"integer","minimum":1,"maximum":100,"default":5},"level":{"type":"integer","minimum":0,"maximum":100,"default":0},"include_children":{"type":"boolean","default":true}},"required":["type"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"audio_switch"},"first":{"type":"string"},"second":{"type":"string"},"input":{"type":"boolean"},"first_label":{"type":"string"},"second_label":{"type":"string"},"first_icon":{"type":"string"},"second_icon":{"type":"string"}},"required":["type","first","second","input"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"if_window"},"title":{"type":"string"},"then_steps":nested,"else_steps":nested},"required":["type","title","then_steps","else_steps"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"toggle"},"a":nested,"b":nested},"required":["type","a","b"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"repeat"},"count":{"type":"integer","minimum":1,"maximum":100},"steps":nested},"required":["type","count","steps"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"set_variable"},"persistent":{"type":"boolean","default":false},"name":{"type":"string"},"value":{"type":"string"}},"required":["type","name","value"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"if_variable"},"name":{"type":"string"},"value":{"type":"string"},"then_steps":nested,"else_steps":nested},"required":["type","name","value","then_steps","else_steps"],"additionalProperties":false}),
        json!({"type":"object","properties":{"type":{"const":"auto_hotkey"},"executable":{"type":"string"},"script":{"type":"string","maxLength":200000},"args":{"type":"array","items":{"type":"string"},"maxItems":128}},"required":["type","executable","script","args"],"additionalProperties":false})
    ]);
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"math"},"persistent":{"type":"boolean","default":false},"name":{"type":"string","minLength":1,"maxLength":64,"default":"compteur"},"op":{"enum":["set","add","subtract","multiply","divide","modulo","min","max","random"],"default":"add"},"value":{"type":"string","maxLength":4096,"default":"1"},"min":{"type":"number","default":0},"max":{"type":"number","default":100}},"required":["type"],"additionalProperties":false}));
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"text_op"},"persistent":{"type":"boolean","default":false},"name":{"type":"string","minLength":1,"maxLength":64,"default":"texte"},"op":{"enum":["set","append","prepend","upper","lower","trim","replace","length"],"default":"upper"},"value":{"type":"string","maxLength":4096,"default":""},"find":{"type":"string","maxLength":4096,"default":""}},"required":["type"],"additionalProperties":false}));
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"clipboard"},"op":{"enum":["copy_text","read_to_variable","clear","save","restore"],"default":"copy_text"},"value":{"type":"string","maxLength":100000,"default":""},"name":{"type":"string","minLength":1,"maxLength":64,"default":"texte"}},"required":["type"],"additionalProperties":false}));
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"key_hold"},"keys":{"type":"string","default":"CTRL"},"action":{"enum":["down","up"],"default":"down"},"hold_ms":{"type":"integer","minimum":0,"maximum":3600000,"default":0}},"required":["type"],"additionalProperties":false}));
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"window_op"},"target":{"enum":["foreground","title","exe"],"default":"foreground"},"value":{"type":"string","maxLength":260,"default":""},"op":{"enum":["activate","minimize","maximize","restore","move","resize","move_to_monitor","toggle_always_on_top","always_on_top_on","always_on_top_off"],"default":"maximize"},"x":{"type":"integer","minimum":-32768,"maximum":32767,"default":0},"y":{"type":"integer","minimum":-32768,"maximum":32767,"default":0},"w":{"type":"integer","minimum":1,"maximum":32767,"default":800},"h":{"type":"integer","minimum":1,"maximum":32767,"default":600},"monitor":{"type":"integer","minimum":1,"maximum":16,"default":2}},"required":["type"],"additionalProperties":false}));
    let condition_schema = json!({"oneOf":[
        {"type":"object","properties":{"kind":{"const":"foreground_app"},"exe":{"type":"string"}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"app_running"},"exe":{"type":"string"}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"window_open"},"title":{"type":"string"}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"enum":["app_audio_playing","app_muted"]},"exe":{"type":"string"}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"app_volume_compare"},"exe":{"type":"string"},"op":{"enum":["above","below"]},"value":{"type":"integer","minimum":0,"maximum":100}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"device_muted"},"input":{"type":"boolean"},"device":{"type":"string"}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"device_volume_compare"},"input":{"type":"boolean"},"device":{"type":"string"},"op":{"enum":["above","below"]},"value":{"type":"integer","minimum":0,"maximum":100}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"audio_device_present"},"name_contains":{"type":"string"},"input":{"type":"boolean"}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"default_output_is"},"name_contains":{"type":"string"}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"variable_compare"},"name":{"type":"string"},"op":{"enum":["equals","not_equals","contains","greater","less"]},"value":{"type":"string"}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"not"},"condition":{"$ref":"#/$defs/condition"}},"required":["kind","condition"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"all"},"conditions":{"type":"array","items":{"$ref":"#/$defs/condition"},"minItems":1,"maxItems":32}},"required":["kind"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"const":"any"},"conditions":{"type":"array","items":{"$ref":"#/$defs/condition"},"minItems":1,"maxItems":32}},"required":["kind"],"additionalProperties":false}
    ]});
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"notify"},"title":{"type":"string","minLength":1,"maxLength":63,"default":"DeckForge"},"message":{"type":"string","minLength":1,"maxLength":2048}},"required":["type","message"],"additionalProperties":false}));
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"wait_until"},"condition":{"$ref":"#/$defs/condition"},"timeout_ms":{"type":"integer","minimum":1,"maximum":600000,"default":5000},"poll_ms":{"type":"integer","minimum":50,"maximum":600000,"default":200}},"required":["type"],"additionalProperties":false}));
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"if_condition"},"condition":{"$ref":"#/$defs/condition"},"then_steps":nested.clone(),"else_steps":nested.clone()},"required":["type"],"additionalProperties":false}));
    step_schema["oneOf"].as_array_mut().unwrap().push(json!({"type":"object","properties":{"type":{"const":"disabled"},"step":{"$ref":"#/$defs/step"}},"required":["type","step"],"additionalProperties":false}));
    let key_args = json!({"type":"object","properties":{"profile":profile,"page":{"type":"string","maxLength":160},"key":position},"required":["profile","key"],"additionalProperties":false});
    let mut list=vec![
        tool("deckforge_get_state","Read connection state and current profiles. Available only when Assistant IA is enabled.",json!({"type":"object","properties":{},"additionalProperties":false}),true),
        tool("deckforge_get_profile","Read one profile including its key bindings.",json!({"type":"object","properties":{"profile":profile},"required":["profile"],"additionalProperties":false}),true),
        tool("deckforge_create_profile","Propose a new empty profile. Requires approval in DeckForge; never overwrites an existing profile.",json!({"type":"object","properties":{"name":profile},"required":["name"],"additionalProperties":false}),false),
        tool("deckforge_configure_key","Propose a complete binding for a key. The user must review and apply it in DeckForge. Key positions are 1 through 15, top to bottom, left to right. No arbitrary shell API is exposed; a launch step runs a configured program only after approval and a later user action.",json!({"type":"object","properties":{"profile":profile,"page":{"type":"string","maxLength":160},"key":position,"label":{"type":"string","maxLength":128},"background":{"type":"boolean","description":"true uses the saved RGB color; false uses a black LCD background while preserving the color"},"color":{"type":"array","items":{"type":"integer","minimum":0,"maximum":255},"minItems":3,"maxItems":3},"repeat":{"type":"integer","minimum":1,"maximum":1000},"icon_scale":{"type":"integer","minimum":20,"maximum":200},"state_display":{"type":"object","properties":{"condition":{"$ref":"#/$defs/condition"},"on":{"$ref":"#/$defs/state_face"},"off":{"$ref":"#/$defs/state_face"},"poll_ms":{"type":"integer","minimum":250,"maximum":600000,"default":1000}},"additionalProperties":false},"plugin":{"type":"object","properties":{"plugin":{"type":"string","minLength":1},"action":{"type":"string","minLength":1},"settings":{},"state":{"type":"integer","minimum":0,"maximum":255}},"required":["plugin","action"],"additionalProperties":false},"steps":{"type":"array","items":step_schema.clone(),"maxItems":256},"long_steps":{"type":"array","items":step_schema,"maxItems":256}},"required":["profile","key","label","steps"],"additionalProperties":false}),false),
        tool("deckforge_simulate_macro","Simulate one iteration without keyboard, application, timing or profile side effects. Does not require approval.",key_args.clone(),true),
        tool("deckforge_request_run","Ask the user in DeckForge to execute an already configured macro. Returns a proposal id; it does not execute until approved. Approval starts a 3-second delay.",key_args,false),
        tool("deckforge_stop_macro","Immediately cancel the current macro and pause physical keys.",json!({"type":"object","properties":{},"additionalProperties":false}),false),
        tool("deckforge_proposal_status","Read whether a proposal is pending, approved, rejected or stale.",json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false}),true),
    ];
    list.push(tool("deckforge_get_audio_devices","Read available Windows audio endpoints and their active IDs. No audio settings are changed.",json!({"type":"object","properties":{},"additionalProperties":false}),true));
    list.push(tool("deckforge_get_icons","Search locally installed icon packs (no file contents returned). Audio icons accept builtin:headphones, builtin:speakers, builtin:microphone, or a returned PNG/JPEG/SVG path. Empty first_icon/second_icon use distinct default icons.",json!({"type":"object","properties":{"query":{"type":"string","maxLength":128},"limit":{"type":"integer","minimum":1,"maximum":50}},"additionalProperties":false}),true));
    list.push(tool("deckforge_create_page","Propose a new page or folder in a profile. Requires approval in DeckForge. Folders have a Back key; use configure_key to link a folder after approval.",json!({"type":"object","properties":{"profile":profile,"id":{"type":"string","minLength":1,"maxLength":160},"name":{"type":"string","minLength":1,"maxLength":128},"folder":{"type":"boolean"}},"required":["profile","id","name","folder"],"additionalProperties":false}),false));
    for tool in &mut list {
        if tool["name"] == "deckforge_configure_key" {
            tool["inputSchema"]["$defs"] = json!({"step":step_schema,"condition":condition_schema,"state_face":{"type":"object","properties":{"label":{"type":"string","maxLength":128},"image":{"type":"string","maxLength":1024},"color":{"type":"array","items":{"type":"integer","minimum":0,"maximum":255},"minItems":3,"maxItems":3}},"additionalProperties":false}});
        }
    }
    json!({"tools":list})
}
fn tool(name: &str, description: &str, input_schema: Value, read: bool) -> Value {
    json!({"name":name,"description":description,"inputSchema":input_schema,"annotations":{"readOnlyHint":read,"destructiveHint":!read,"idempotentHint":read,"openWorldHint":false}})
}
pub fn initialize(requested: Option<&str>) -> Value {
    let version = match requested {
        Some("2024-11-05") => "2024-11-05",
        Some("2025-03-26") => "2025-03-26",
        _ => VERSION,
    };
    json!({"protocolVersion":version,"serverInfo":{"name":"DeckForge","version":env!("CARGO_PKG_VERSION")},"capabilities":{"tools":{"listChanged":false}},"instructions":"Enable Assistant IA in DeckForge. Changes and real macro execution require review inside DeckForge. Use create_profile then configure_key after the profile proposal is approved. Never claim a pending proposal has already been applied."})
}
pub fn success(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
pub fn tool_result(result: Result<Value, String>) -> Value {
    match result {
        Ok(v) => {
            json!({"content":[{"type":"text","text":v.to_string()}],"structuredContent":v,"isError":false})
        }
        Err(e) => json!({"content":[{"type":"text","text":e}],"isError":true}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tools_have_strict_structured_inputs() {
        for tool in tools()["tools"].as_array().unwrap() {
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
            assert!(tool["name"].as_str().unwrap().starts_with("deckforge_"));
        }
    }
    #[test]
    fn soundboard_steps_are_advertised_without_secret_fields() {
        let list = tools();
        let configure = list["tools"].as_array().unwrap().iter()
            .find(|tool| tool["name"] == "deckforge_configure_key").unwrap();
        let variants = configure["inputSchema"]["$defs"]["step"]["oneOf"].as_array().unwrap();
        for kind in ["play_sound", "stop_sounds"] {
            let schema = variants.iter().find(|v| v["properties"]["type"]["const"] == kind).unwrap();
            assert_eq!(schema["additionalProperties"], false);
            assert_eq!(schema["required"][0], "type");
        }
        assert_eq!(variants.iter().find(|v| v["properties"]["type"]["const"] == "play_sound").unwrap()["properties"]["volume"]["maximum"], 100);
    }
    #[test]
    fn newer_clients_get_a_supported_protocol() {
        assert_eq!(initialize(Some("2025-11-25"))["protocolVersion"], VERSION);
        assert_eq!(
            initialize(Some("2024-11-05"))["protocolVersion"],
            "2024-11-05"
        );
    }
}

#[cfg(test)]
mod key_hold_schema_tests {
    #[test]
    fn key_hold_has_strict_schema() {
        let tools = super::tools();
        let configure = tools["tools"].as_array().unwrap().iter().find(|t| t["name"] == "deckforge_configure_key").unwrap();
        let steps = configure["inputSchema"]["$defs"]["step"]["oneOf"].as_array().unwrap();
        let schema = steps.iter().find(|s| s["properties"]["type"]["const"] == "key_hold").unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], serde_json::json!(["type"]));
        assert_eq!(schema["properties"]["hold_ms"]["default"], 0);
    }
}

#[cfg(test)]
mod clipboard_schema_tests {
    #[test]
    fn clipboard_schema_is_strict() {
        let tools = super::tools();
        let configure = tools["tools"].as_array().unwrap().iter().find(|t| t["name"] == "deckforge_configure_key").unwrap();
        let steps = configure["inputSchema"]["$defs"]["step"]["oneOf"].as_array().unwrap();
        let schema = steps.iter().find(|s| s["properties"]["type"]["const"] == "clipboard").unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], serde_json::json!(["type"]));
        assert_eq!(schema["properties"]["op"]["default"], "copy_text");
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub label: String,
    pub title: String,
    pub url: String,
    pub visible: bool,
    pub focused: bool,
    pub maximized: bool,
    pub minimized: bool,
    pub fullscreen: bool,
    pub position: (i32, i32),
    pub size: (u32, u32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomSnapshot {
    pub webview_label: String,
    pub elements: Vec<DomElement>,
    pub ref_map: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomElement {
    pub ref_id: String,
    pub tag: String,
    pub role: Option<String>,
    pub name: Option<String>,
    pub text: Option<String>,
    pub value: Option<String>,
    pub enabled: bool,
    pub visible: bool,
    pub focusable: bool,
    pub bounds: Option<ElementBounds>,
    pub children: Vec<DomElement>,
    pub attributes: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl DomSnapshot {
    pub fn to_accessible_text(&self, indent: usize) -> String {
        let mut output = String::new();
        for element in &self.elements {
            Self::format_element(&mut output, element, indent);
        }
        output
    }

    fn format_element(output: &mut String, element: &DomElement, indent: usize) {
        if !element.visible {
            return;
        }

        let prefix = "  ".repeat(indent);

        let role_str = element.role.as_deref().unwrap_or(&element.tag);
        let name_str = element
            .name
            .as_ref()
            .map(|n| format!(" \"{}\"", n))
            .unwrap_or_default();
        let ref_str = if element.focusable || element.tag == "button" || element.tag == "input" {
            format!(" [ref={}]", element.ref_id)
        } else {
            String::new()
        };

        let line = format!("{}- {}{}{}\n", prefix, role_str, name_str, ref_str);
        output.push_str(&line);

        for child in &element.children {
            Self::format_element(output, child, indent + 1);
        }
    }
}

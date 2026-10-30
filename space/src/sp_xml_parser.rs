//! Port of `SPXmlParser` (expat replaced by quick-xml).

use std::collections::BTreeMap;

use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPXmlNode {
    pub name: String,
    /// All character data directly inside this element, concatenated.
    pub text: String,
    pub attributes: BTreeMap<String, String>,
    pub children: Vec<SPXmlNode>,
    pub text_node: bool,
}

impl SPXmlNode {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(String::as_str)
    }
}

fn append_text(stack: &mut [SPXmlNode], text: &str) {
    let Some(parent) = stack.last_mut() else {
        return;
    };
    if text.is_empty() {
        return;
    }
    parent.text.push_str(text);
    if !parent.children.last().is_some_and(|child| child.text_node) {
        parent.children.push(SPXmlNode { text_node: true, ..Default::default() });
    }
    parent.children.last_mut().unwrap().text.push_str(text);
}

fn start_node(element: &quick_xml::events::BytesStart) -> Result<SPXmlNode, String> {
    let mut node =
        SPXmlNode { name: String::from_utf8_lossy(element.name().as_ref()).into_owned(), ..Default::default() };
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|e| e.to_string())?;
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        let value = attribute.unescape_value().map_err(|e| e.to_string())?.into_owned();
        node.attributes.entry(key).or_insert(value);
    }
    Ok(node)
}

/// Parses a document into its root element. A DOCTYPE stops parsing (as the
/// C++ parser refuses entity declarations), which makes the document invalid.
pub fn parse_sp_xml(xml: &str) -> Result<SPXmlNode, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<SPXmlNode> = Vec::new();
    let mut root: Option<SPXmlNode> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                if root.is_some() {
                    return Err("junk after document element".into());
                }
                stack.push(start_node(&element)?);
            }
            Ok(Event::Empty(element)) => {
                if root.is_some() {
                    return Err("junk after document element".into());
                }
                let node = start_node(&element)?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Ok(Event::End(_)) => {
                let node = stack.pop().ok_or("mismatched tag")?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Ok(Event::Text(text)) => {
                let text = text.unescape().map_err(|e| e.to_string())?;
                if stack.is_empty() {
                    if !text.trim().is_empty() {
                        return Err("syntax error".into());
                    }
                } else {
                    append_text(&mut stack, &text);
                }
            }
            Ok(Event::CData(data)) => {
                let text = String::from_utf8_lossy(&data).into_owned();
                append_text(&mut stack, &text);
            }
            Ok(Event::DocType(_)) => return Err("DOCTYPE is not allowed".into()),
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    if !stack.is_empty() {
        return Err("unclosed token".into());
    }
    root.filter(|node| !node.name.is_empty()).ok_or_else(|| "XML document has no root element".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_elements_attributes_and_text() {
        let root = parse_sp_xml(r#"<vbox id="a"><text size="12">Hello &amp; bye</text><hbox/></vbox>"#).unwrap();
        assert_eq!(root.name, "vbox");
        assert_eq!(root.attr("id"), Some("a"));
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].text, "Hello & bye");
        assert!(root.children[0].children[0].text_node);
        assert_eq!(root.children[1].name, "hbox");
    }

    #[test]
    fn records_text_nodes_between_elements() {
        let root = parse_sp_xml("<a>x<b/>y</a>").unwrap();
        assert_eq!(root.text, "xy");
        assert_eq!(root.children.len(), 3);
        assert!(root.children[0].text_node && root.children[2].text_node);
    }

    #[test]
    fn rejects_invalid_documents() {
        assert!(parse_sp_xml("").is_err());
        assert!(parse_sp_xml("<a>").is_err());
        assert!(parse_sp_xml("<a></b>").is_err());
        assert!(parse_sp_xml("<!DOCTYPE a [<!ENTITY x 'y'>]><a>&x;</a>").is_err());
        assert!(parse_sp_xml("<a/><b/>").is_err());
    }
}

#![forbid(unsafe_code)]

use std::collections::BTreeSet;

/// Stable identifier for one semantic object during its runtime lifetime.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeId(pub u128);

/// Origin of an accessibility object or event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceDomain {
    Native,
    Windows,
    Android,
    Darwin,
    VirtualMachine,
    SystemRecovery,
}

/// Normalized semantic role exposed to assistive technology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Application,
    Window,
    Dialog,
    Alert,
    Button,
    CheckBox,
    ComboBox,
    Edit,
    Heading,
    Image,
    Link,
    List,
    ListItem,
    Menu,
    MenuItem,
    ProgressBar,
    RadioButton,
    Slider,
    StaticText,
    StatusBar,
    Tab,
    TabList,
    Table,
    TableCell,
    Terminal,
    ToolBar,
    Tree,
    TreeItem,
    Unknown,
}

/// Normalized state flags.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum State {
    Busy,
    Checked,
    Collapsed,
    Disabled,
    Expanded,
    Focusable,
    Focused,
    Invalid,
    Modal,
    MultiLine,
    Offscreen,
    Pressed,
    ReadOnly,
    Required,
    Selected,
    Unavailable,
}

/// Actions that an accessibility node can expose.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Action {
    Focus,
    Invoke,
    Expand,
    Collapse,
    Check,
    Uncheck,
    Increment,
    Decrement,
    ScrollForward,
    ScrollBackward,
    SetValue,
}

/// Privacy classification used before data reaches speech, braille, logs or telemetry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sensitivity {
    Public,
    Personal,
    Secret,
}

/// Inclusive-exclusive text range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn is_valid_for(self, text_len: usize) -> bool {
        self.start <= self.end && self.end <= text_len
    }
}

/// Text information for controls that are allowed to expose their content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextInfo {
    pub content: String,
    pub caret: Option<usize>,
    pub selections: Vec<TextRange>,
    pub editable: bool,
}

/// One normalized accessibility object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub id: RuntimeId,
    pub parent: Option<RuntimeId>,
    pub source: SourceDomain,
    pub role: Role,
    pub name: String,
    pub description: Option<String>,
    pub value: Option<String>,
    pub states: BTreeSet<State>,
    pub actions: BTreeSet<Action>,
    pub text: Option<TextInfo>,
    pub sensitivity: Sensitivity,
}

/// Priority used by downstream speech/braille/event consumers.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EventPriority {
    Background,
    Normal,
    Important,
    Critical,
}

/// Normalized accessibility event type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    ObjectCreated,
    ObjectDestroyed,
    FocusChanged,
    NameChanged,
    DescriptionChanged,
    ValueChanged,
    StateChanged,
    TextInserted,
    TextRemoved,
    TextReplaced,
    CaretMoved,
    SelectionChanged,
    LiveRegionChanged,
    Notification,
    WindowActivated,
    WindowDeactivated,
}

/// Optional non-secret text delta for text events.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextDelta {
    pub range: TextRange,
    pub text: String,
}

/// Ordered normalized event emitted by an adapter or first-party shell component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityEvent {
    pub sequence: u64,
    pub monotonic_ns: u64,
    pub node_id: RuntimeId,
    pub source: SourceDomain,
    pub kind: EventKind,
    pub priority: EventPriority,
    pub sensitivity: Sensitivity,
    pub text_delta: Option<TextDelta>,
}

/// Validation error returned before semantic data enters the broker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    SecretNodeContainsValue,
    SecretNodeContainsText,
    CaretOutOfRange,
    SelectionOutOfRange,
    SecretEventContainsText,
}

/// Validate a node at the accessibility broker boundary.
///
/// Secret fields are allowed to expose role/state/action metadata, but never their
/// raw value or text content.
pub fn validate_node(node: &Node) -> Result<(), ValidationError> {
    if node.sensitivity == Sensitivity::Secret {
        if node.value.is_some() {
            return Err(ValidationError::SecretNodeContainsValue);
        }
        if node.text.is_some() {
            return Err(ValidationError::SecretNodeContainsText);
        }
    }

    if let Some(text) = &node.text {
        let len = text.content.len();
        if text.caret.is_some_and(|caret| caret > len) {
            return Err(ValidationError::CaretOutOfRange);
        }
        if text
            .selections
            .iter()
            .any(|range| !range.is_valid_for(len))
        {
            return Err(ValidationError::SelectionOutOfRange);
        }
    }

    Ok(())
}

/// Validate an event before it is published to consumers.
pub fn validate_event(event: &AccessibilityEvent) -> Result<(), ValidationError> {
    if event.sensitivity == Sensitivity::Secret && event.text_delta.is_some() {
        return Err(ValidationError::SecretEventContainsText);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_node() -> Node {
        Node {
            id: RuntimeId(1),
            parent: None,
            source: SourceDomain::Native,
            role: Role::Edit,
            name: "Editor".to_owned(),
            description: None,
            value: None,
            states: BTreeSet::from([State::Focusable, State::Focused]),
            actions: BTreeSet::from([Action::Focus, Action::SetValue]),
            text: Some(TextInfo {
                content: "hello".to_owned(),
                caret: Some(5),
                selections: vec![TextRange::new(0, 5)],
                editable: true,
            }),
            sensitivity: Sensitivity::Public,
        }
    }

    #[test]
    fn valid_public_text_node_passes() {
        assert_eq!(validate_node(&base_node()), Ok(()));
    }

    #[test]
    fn secret_node_cannot_expose_text() {
        let mut node = base_node();
        node.sensitivity = Sensitivity::Secret;
        assert_eq!(
            validate_node(&node),
            Err(ValidationError::SecretNodeContainsText)
        );
    }

    #[test]
    fn secret_node_cannot_expose_value() {
        let mut node = base_node();
        node.text = None;
        node.value = Some("password".to_owned());
        node.sensitivity = Sensitivity::Secret;
        assert_eq!(
            validate_node(&node),
            Err(ValidationError::SecretNodeContainsValue)
        );
    }

    #[test]
    fn caret_must_be_inside_text() {
        let mut node = base_node();
        node.text.as_mut().expect("text fixture").caret = Some(6);
        assert_eq!(validate_node(&node), Err(ValidationError::CaretOutOfRange));
    }

    #[test]
    fn selection_must_be_inside_text() {
        let mut node = base_node();
        node.text.as_mut().expect("text fixture").selections = vec![TextRange::new(1, 6)];
        assert_eq!(
            validate_node(&node),
            Err(ValidationError::SelectionOutOfRange)
        );
    }

    #[test]
    fn secret_event_cannot_carry_text_delta() {
        let event = AccessibilityEvent {
            sequence: 1,
            monotonic_ns: 10,
            node_id: RuntimeId(42),
            source: SourceDomain::Windows,
            kind: EventKind::TextInserted,
            priority: EventPriority::Important,
            sensitivity: Sensitivity::Secret,
            text_delta: Some(TextDelta {
                range: TextRange::new(0, 1),
                text: "x".to_owned(),
            }),
        };

        assert_eq!(
            validate_event(&event),
            Err(ValidationError::SecretEventContainsText)
        );
    }
}

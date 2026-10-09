//! Package Input: a form, an ordered set of fields with inline validation.
//!
//! ```text
//!   Name
//!   › nightly-report▏
//!   Project
//!     —
//!   ✕ Choose a project.
//!   Notify me
//!     ○ Off
//! ```
//!
//! A [`FormState`] holds the fields, which one has focus and what each one's
//! check last said. A [`Form`] paints it. The host owns what a submit means:
//! [`FormState::handle_key`] only says [`FormOutcome::Submitted`] when every
//! field passes, so the host never sees a submit of invalid data.
//!
//! Three kinds of field ([`FormFieldKind`]): a text field (one
//! [`TextInputState`], or a secret), a checkbox, and a read-only row. The
//! last is shown but never takes focus.
//!
//! Validation is a plain function the caller supplies,
//! `fn(&str) -> Result<(), Cow<'static, str>>` ([`FormValidator`]); the error
//! is the words shown, so the kit owns none of them. When it runs:
//!
//! - **On blur** (focus leaves the field) and **on submit** (every field): the
//!   result is shown. An error appears as `✕` and its words under the field,
//!   never as color alone.
//! - **On every edit**, quietly: an error already shown clears the moment the
//!   text becomes valid (or changes to the new reason), but a field that was
//!   not showing an error does not start showing one while the person is
//!   still typing.
//! - A submit that fails clears nothing, moves focus to the first invalid
//!   field and answers [`FormOutcome::Blocked`] with its index.
//!
//! Keys: `Tab`, `↓` and `Shift+Tab`, `↑` move focus (wrapping, skipping
//! read-only rows), `Space` toggles a checkbox, `Enter` submits and `Esc`
//! cancels. Everything else goes to the focused text field
//! ([`TextInputState::handle_key`]).

use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
};

use super::text_input::{FieldChrome, FieldNote, KeyEffect, paint_input_value};
use crate::{Paint, Role, Theme, glyphs, text};

/// A caller's check of a text field: `Ok`, or the words that say what is
/// wrong. Plain function so a field stays `Clone` and `Debug`.
pub type FormValidator = fn(&str) -> Result<(), Cow<'static, str>>;

/// What a field's check last said.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FormStatus {
    /// Not checked yet, or edited since and not worth saying: nothing shows.
    #[default]
    Untouched,
    /// Checked and passed. Nothing shows: a field with no complaint is fine.
    Valid,
    /// Checked and failed: `✕` and these words show under the field.
    Invalid(Cow<'static, str>),
}

/// What kind of field, and its value.
#[derive(Clone, Debug)]
pub enum FormFieldKind {
    Text(crate::TextInputState),
    Check(bool),
    ReadOnly(Cow<'static, str>),
}

/// One field of a [`FormState`], built with [`FormField::text`],
/// [`FormField::secret`], [`FormField::check`] or [`FormField::read_only`].
#[derive(Clone, Debug)]
pub struct FormField {
    label: Cow<'static, str>,
    help: Option<Cow<'static, str>>,
    placeholder: Option<Cow<'static, str>>,
    kind: FormFieldKind,
    validate: Option<FormValidator>,
    /// A checkbox that must be on, and the words when it is not.
    required: Option<Cow<'static, str>>,
    status: FormStatus,
    initial_text: String,
    initial_checked: bool,
}

impl FormField {
    fn new(label: impl Into<Cow<'static, str>>, kind: FormFieldKind) -> Self {
        Self {
            label: label.into(),
            help: None,
            placeholder: None,
            kind,
            validate: None,
            required: None,
            status: FormStatus::Untouched,
            initial_text: String::new(),
            initial_checked: false,
        }
    }

    /// A single-line text field.
    #[must_use]
    pub fn text(label: impl Into<Cow<'static, str>>) -> Self {
        Self::new(label, FormFieldKind::Text(crate::TextInputState::new()))
    }

    /// A secret: painted as a mask, never as its text.
    #[must_use]
    pub fn secret(label: impl Into<Cow<'static, str>>) -> Self {
        Self::new(label, FormFieldKind::Text(crate::TextInputState::secret()))
    }

    /// A checkbox.
    #[must_use]
    pub fn check(label: impl Into<Cow<'static, str>>, checked: bool) -> Self {
        let mut field = Self::new(label, FormFieldKind::Check(checked));
        field.initial_checked = checked;
        field
    }

    /// A row that shows a value and takes no focus.
    #[must_use]
    pub fn read_only(
        label: impl Into<Cow<'static, str>>,
        value: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(label, FormFieldKind::ReadOnly(value.into()))
    }

    /// The starting text of a text field (what [`FormState::is_dirty`]
    /// compares against).
    #[must_use]
    pub fn value(mut self, text: &str) -> Self {
        if let FormFieldKind::Text(input) = &mut self.kind {
            input.set_text(text);
            self.initial_text = input.text().to_owned();
        }
        self
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<Cow<'static, str>>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Muted guidance under the field, shown while it has no error.
    #[must_use]
    pub fn help(mut self, help: impl Into<Cow<'static, str>>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// The check a text field runs on blur and on submit.
    #[must_use]
    pub fn validate(mut self, validate: FormValidator) -> Self {
        self.validate = Some(validate);
        self
    }

    /// A checkbox that blocks submit until it is on, with the words to show.
    #[must_use]
    pub fn require(mut self, message: impl Into<Cow<'static, str>>) -> Self {
        self.required = Some(message.into());
        self
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> &FormFieldKind {
        &self.kind
    }

    #[must_use]
    pub const fn status(&self) -> &FormStatus {
        &self.status
    }

    /// The text of a text field.
    #[must_use]
    pub fn text_value(&self) -> Option<&str> {
        match &self.kind {
            FormFieldKind::Text(input) => Some(input.text()),
            _ => None,
        }
    }

    /// The state of a checkbox.
    #[must_use]
    pub const fn checked(&self) -> Option<bool> {
        match self.kind {
            FormFieldKind::Check(on) => Some(on),
            _ => None,
        }
    }

    const fn focusable(&self) -> bool {
        !matches!(self.kind, FormFieldKind::ReadOnly(_))
    }

    const fn has_rule(&self) -> bool {
        self.validate.is_some() || self.required.is_some()
    }

    fn check_now(&self) -> Result<(), Cow<'static, str>> {
        match &self.kind {
            FormFieldKind::Text(input) => self.validate.map_or(Ok(()), |check| check(input.text())),
            FormFieldKind::Check(on) => match (&self.required, on) {
                (Some(message), false) => Err(message.clone()),
                _ => Ok(()),
            },
            FormFieldKind::ReadOnly(_) => Ok(()),
        }
    }

    fn is_dirty(&self) -> bool {
        match &self.kind {
            FormFieldKind::Text(input) => input.text() != self.initial_text,
            FormFieldKind::Check(on) => *on != self.initial_checked,
            FormFieldKind::ReadOnly(_) => false,
        }
    }
}

/// What a key did to a [`FormState`]: the message a host reacts to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormOutcome {
    /// The key means nothing to the form; the host may use it.
    Ignored,
    /// Something changed (text, a cursor, a checkbox, focus, an error
    /// shown or cleared); repaint.
    Changed,
    /// Enter, and every field passes.
    Submitted,
    /// Enter, but field `n` is invalid. Its error is showing and it has
    /// focus; nothing was cleared.
    Blocked(usize),
    /// Esc. [`FormState::is_dirty`] says whether there is anything to lose.
    Cancelled,
}

/// The fields of a form, which one has focus and what each check said.
#[derive(Clone, Debug, Default)]
pub struct FormState {
    fields: Vec<FormField>,
    focus: usize,
}

impl FormState {
    /// A form over `fields`, focus on the first one that takes it.
    #[must_use]
    pub fn new(fields: Vec<FormField>) -> Self {
        let focus = fields.iter().position(FormField::focusable).unwrap_or(0);
        Self { fields, focus }
    }

    #[must_use]
    pub fn fields(&self) -> &[FormField] {
        &self.fields
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    #[must_use]
    pub fn field(&self, index: usize) -> Option<&FormField> {
        self.fields.get(index)
    }

    /// The index of the focused field.
    #[must_use]
    pub const fn focus(&self) -> usize {
        self.focus
    }

    /// The text of field `index`, if it is a text field.
    #[must_use]
    pub fn text(&self, index: usize) -> Option<&str> {
        self.fields.get(index)?.text_value()
    }

    /// The state of field `index`, if it is a checkbox.
    #[must_use]
    pub fn checked(&self, index: usize) -> Option<bool> {
        self.fields.get(index)?.checked()
    }

    #[must_use]
    pub fn status(&self, index: usize) -> Option<&FormStatus> {
        self.fields.get(index).map(FormField::status)
    }

    /// Whether any field differs from what it started as.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.fields.iter().any(FormField::is_dirty)
    }

    /// Show `status` on field `index`: how a host reports what only it can
    /// know (the name is taken, the project is gone).
    pub fn set_status(&mut self, index: usize, status: FormStatus) {
        if let Some(field) = self.fields.get_mut(index) {
            field.status = status;
        }
    }

    /// Move focus to `index` (ignored for a read-only row or an index past
    /// the end). The field it leaves is checked, as on any blur.
    pub fn set_focus(&mut self, index: usize) {
        if self.fields.get(index).is_some_and(FormField::focusable) && index != self.focus {
            self.blur();
            self.focus = index;
        }
    }

    /// Run the checks of every field now; the index of the first invalid one.
    pub fn validate_all(&mut self) -> Option<usize> {
        let mut first = None;
        for (i, field) in self.fields.iter_mut().enumerate() {
            if !field.has_rule() {
                continue;
            }
            field.status = match field.check_now() {
                Ok(()) => FormStatus::Valid,
                Err(reason) => {
                    first.get_or_insert(i);
                    FormStatus::Invalid(reason)
                }
            };
        }
        first
    }

    /// Whether every field passes its check right now. Shows nothing.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.fields.iter().all(|f| f.check_now().is_ok())
    }

    /// Submit: check every field, and either answer
    /// [`FormOutcome::Submitted`] or put focus on the first invalid field
    /// and answer [`FormOutcome::Blocked`].
    pub fn submit(&mut self) -> FormOutcome {
        match self.validate_all() {
            None => FormOutcome::Submitted,
            Some(first) => {
                self.focus = first;
                FormOutcome::Blocked(first)
            }
        }
    }

    /// Insert pasted text into the focused text field.
    pub fn paste(&mut self, text: &str) -> FormOutcome {
        let Some(FormFieldKind::Text(input)) = self.fields.get_mut(self.focus).map(|f| &mut f.kind)
        else {
            return FormOutcome::Ignored;
        };
        if input.paste(text) == crate::TextInputOutcome::Ignored {
            return FormOutcome::Ignored;
        }
        self.after_edit();
        FormOutcome::Changed
    }

    /// Apply a key press and say what happened (see the module's key list).
    /// Navigation and editing accept held-key repeats. Submit, cancel and
    /// checkbox toggles require an initial press; modified navigation and
    /// activation keys belong to the host. Releases are ignored.
    pub fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        if key.kind == KeyEventKind::Release || self.fields.is_empty() {
            return FormOutcome::Ignored;
        }
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Tab | KeyCode::BackTab
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.step(key.code == KeyCode::Tab && !shift)
            }
            KeyCode::Down if key.modifiers.is_empty() => self.step(true),
            KeyCode::Up if key.modifiers.is_empty() => self.step(false),
            KeyCode::Enter if key.kind == KeyEventKind::Press && key.modifiers.is_empty() => {
                self.submit()
            }
            KeyCode::Esc if key.kind == KeyEventKind::Press && key.modifiers.is_empty() => {
                FormOutcome::Cancelled
            }
            _ => self.edit(key),
        }
    }

    fn edit(&mut self, key: KeyEvent) -> FormOutcome {
        match self.fields.get_mut(self.focus).map(|f| &mut f.kind) {
            Some(FormFieldKind::Text(input)) => match input.apply(key) {
                KeyEffect::Nothing => FormOutcome::Ignored,
                KeyEffect::Moved => FormOutcome::Changed,
                // Enter and Esc never reach here: the form took them.
                KeyEffect::Submit | KeyEffect::Cancel => FormOutcome::Ignored,
                KeyEffect::Edited => {
                    self.after_edit();
                    FormOutcome::Changed
                }
            },
            Some(FormFieldKind::Check(on))
                if key.code == KeyCode::Char(' ')
                    && key.kind == KeyEventKind::Press
                    && key.modifiers.is_empty() =>
            {
                *on = !*on;
                self.after_edit();
                FormOutcome::Changed
            }
            _ => FormOutcome::Ignored,
        }
    }

    /// Move focus one field, wrapping and skipping read-only rows. Ignored
    /// when there is nowhere to go, so the host can use the key.
    fn step(&mut self, forward: bool) -> FormOutcome {
        let n = self.fields.len();
        let target = (1..n)
            .map(|d| {
                if forward {
                    (self.focus + d) % n
                } else {
                    (self.focus + n - d) % n
                }
            })
            .find(|i| self.fields[*i].focusable());
        match target {
            Some(to) => {
                self.blur();
                self.focus = to;
                FormOutcome::Changed
            }
            None => FormOutcome::Ignored,
        }
    }

    /// Focus is leaving the focused field: show what its check says.
    fn blur(&mut self) {
        if let Some(field) = self.fields.get_mut(self.focus)
            && field.has_rule()
        {
            field.status = match field.check_now() {
                Ok(()) => FormStatus::Valid,
                Err(reason) => FormStatus::Invalid(reason),
            };
        }
    }

    /// The focused field was edited: re-run its check without announcing a
    /// new error. See the module's note on when errors show.
    fn after_edit(&mut self) {
        let Some(field) = self.fields.get_mut(self.focus) else {
            return;
        };
        if !field.has_rule() {
            return;
        }
        let result = field.check_now();
        field.status = match (std::mem::take(&mut field.status), result) {
            (FormStatus::Invalid(_), Err(reason)) => FormStatus::Invalid(reason),
            (FormStatus::Invalid(_) | FormStatus::Valid, Ok(())) => FormStatus::Valid,
            _ => FormStatus::Untouched,
        };
    }
}

/// The words a [`Form`] shows itself. English by default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormWords {
    /// A checkbox that is on.
    pub on: Cow<'static, str>,
    /// A checkbox that is off.
    pub off: Cow<'static, str>,
    /// Under a read-only row.
    pub read_only: Cow<'static, str>,
}

impl Default for FormWords {
    fn default() -> Self {
        Self {
            on: Cow::Borrowed("On"),
            off: Cow::Borrowed("Off"),
            read_only: Cow::Borrowed("Read only"),
        }
    }
}

/// Paints a [`FormState`]: each field as a label, its input and, under it,
/// an error (`✕` and words), help, or what a read-only row is. Fields stack
/// without gaps; when the form is taller than its area it scrolls to keep
/// the focused field in view.
#[derive(Clone, Debug)]
pub struct Form<'a> {
    state: &'a FormState,
    focused: bool,
    words: FormWords,
}

impl<'a> Form<'a> {
    #[must_use]
    pub fn new(state: &'a FormState) -> Self {
        Self {
            state,
            focused: true,
            words: FormWords::default(),
        }
    }

    /// Whether the form has the host's focus. An unfocused form paints no
    /// prompt and no cursor; its state still remembers which field is next.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub fn words(mut self, words: &FormWords) -> Self {
        self.words = words.clone();
        self
    }

    fn chrome<'f>(&'f self, index: usize, field: &'f FormField) -> FieldChrome<'f> {
        let note = match (&field.status, &field.kind) {
            (FormStatus::Invalid(reason), _) => Some(FieldNote::error(reason)),
            (_, FormFieldKind::ReadOnly(_)) => Some(FieldNote::locked(&self.words.read_only)),
            _ => field.help.as_deref().map(FieldNote::help),
        };
        FieldChrome {
            label: Some(&field.label),
            focused: self.focused && index == self.state.focus && field.focusable(),
            dimmed: matches!(field.kind, FormFieldKind::ReadOnly(_)),
            note,
        }
    }

    /// Each visible field and the rows it gets in `area`. A form taller than
    /// `area` starts at the first field from which the focused one still
    /// fits.
    fn layout(&self, area: Rect) -> Vec<(usize, Rect)> {
        let heights: Vec<u16> = self
            .state
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| self.chrome(i, f).height(area.width))
            .collect();
        let focus = self.state.focus.min(heights.len().saturating_sub(1));
        let mut first = focus;
        let mut used = heights.get(focus).copied().unwrap_or(0);
        while first > 0 && used.saturating_add(heights[first - 1]) <= area.height {
            first -= 1;
            used = used.saturating_add(heights[first]);
        }
        let mut y = area.y;
        let mut placed = Vec::new();
        for (i, height) in heights.iter().enumerate().skip(first) {
            let room = area.bottom().saturating_sub(y);
            if room == 0 {
                break;
            }
            let height = (*height).min(room);
            placed.push((i, Rect { y, height, ..area }));
            y += height;
        }
        placed
    }

    /// Where the terminal's own cursor goes when the form is painted in
    /// `area`: the cursor cell of the focused text field.
    #[must_use]
    pub fn cursor_position(&self, area: Rect) -> Option<Position> {
        let (index, rect) = self
            .layout(area)
            .into_iter()
            .find(|(i, _)| *i == self.state.focus)?;
        let field = &self.state.fields[index];
        let FormFieldKind::Text(input) = &field.kind else {
            return None;
        };
        let placed = self.chrome(index, field).place(rect)?;
        let col = if input.is_empty() {
            0
        } else {
            input.window(usize::from(placed.value.width)).cursor_col?
        };
        let x = placed
            .value
            .x
            .saturating_add(u16::try_from(col).unwrap_or(u16::MAX));
        (self.focused && placed.value.width > 0 && x < placed.value.right())
            .then_some(Position::new(x, placed.value.y))
    }
}

impl Paint for Form<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let area = area.intersection(buf.area);
        for (index, rect) in self.layout(area) {
            let field = &self.state.fields[index];
            let chrome = self.chrome(index, field);
            let (focused, dimmed) = (chrome.focused, chrome.dimmed);
            chrome.paint(rect, buf, theme, |value, buf| match &field.kind {
                FormFieldKind::Text(input) => paint_input_value(
                    input,
                    field.placeholder.as_deref(),
                    focused,
                    dimmed,
                    value,
                    buf,
                    theme,
                ),
                FormFieldKind::Check(on) => self.paint_check(*on, focused, value, buf, theme),
                FormFieldKind::ReadOnly(shown) => {
                    let safe = text::display_safe(shown);
                    let shown = text::truncate(&safe, usize::from(value.width), theme.ascii());
                    buf.set_stringn(
                        value.x,
                        value.y,
                        shown,
                        usize::from(value.width),
                        theme.fg(Role::Muted).add_modifier(Modifier::DIM),
                    );
                }
            });
        }
    }

    fn height(&self, width: u16, _theme: &Theme) -> u16 {
        self.state
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| self.chrome(i, f).height(width))
            .fold(0, u16::saturating_add)
    }
}

impl Form<'_> {
    /// `● On` / `○ Off` (`[x]` / `[ ]` in ASCII, as the picker draws them):
    /// a shape and a word, and bold when focused.
    fn paint_check(&self, on: bool, focused: bool, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let ascii = theme.ascii();
        let (mark, word, role) = match (on, ascii) {
            (true, false) => (glyphs::CURRENT, &self.words.on, Role::Foreground),
            (false, false) => (glyphs::AVAILABLE, &self.words.off, Role::Muted),
            (true, true) => ("[x]", &self.words.on, Role::Foreground),
            (false, true) => ("[ ]", &self.words.off, Role::Muted),
        };
        let mut style = theme.fg(role);
        if focused {
            style = style.add_modifier(Modifier::BOLD);
        }
        let line = format!("{mark} {}", text::display_safe(word));
        let line = text::truncate(&line, usize::from(area.width), ascii);
        buf.set_stringn(area.x, area.y, line, usize::from(area.width), style);
    }
}

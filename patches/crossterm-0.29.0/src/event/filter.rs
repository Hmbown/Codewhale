use crate::event::InternalEvent;

/// Interface for filtering an `InternalEvent`.
pub(crate) trait Filter: Send + Sync + 'static {
    /// Returns whether the given event fulfills the filter.
    fn eval(&self, event: &InternalEvent) -> bool;
}

#[cfg(unix)]
#[derive(Debug, Clone)]
pub(crate) struct CursorPositionFilter;

#[cfg(unix)]
impl Filter for CursorPositionFilter {
    fn eval(&self, event: &InternalEvent) -> bool {
        matches!(*event, InternalEvent::CursorPosition(_, _))
    }
}

#[cfg(unix)]
#[derive(Debug, Clone)]
pub(crate) struct KeyboardEnhancementFlagsFilter;

#[cfg(unix)]
impl Filter for KeyboardEnhancementFlagsFilter {
    fn eval(&self, event: &InternalEvent) -> bool {
        // This filter checks for either a KeyboardEnhancementFlags response or
        // a PrimaryDeviceAttributes response. If we receive the PrimaryDeviceAttributes
        // response but not KeyboardEnhancementFlags, the terminal does not support
        // progressive keyboard enhancement.
        matches!(
            *event,
            InternalEvent::KeyboardEnhancementFlags(_) | InternalEvent::PrimaryDeviceAttributes
        )
    }
}

#[cfg(unix)]
#[derive(Debug, Clone)]
pub(crate) struct PrimaryDeviceAttributesFilter;

#[cfg(unix)]
impl Filter for PrimaryDeviceAttributesFilter {
    fn eval(&self, event: &InternalEvent) -> bool {
        matches!(*event, InternalEvent::PrimaryDeviceAttributes)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EventFilter;

impl Filter for EventFilter {
    #[cfg(unix)]
    fn eval(&self, event: &InternalEvent) -> bool {
        matches!(*event, InternalEvent::Event(_))
    }

    #[cfg(windows)]
    fn eval(&self, _: &InternalEvent) -> bool {
        true
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::{
        super::Event, CursorPositionFilter, EventFilter, Filter, InternalEvent,
        KeyboardEnhancementFlagsFilter, PrimaryDeviceAttributesFilter,
    };

    #[derive(Debug, Clone)]
    pub(crate) struct InternalEventFilter;

    impl Filter for InternalEventFilter {
        fn eval(&self, _: &InternalEvent) -> bool {
            true
        }
    }

    #[test]
    fn test_cursor_position_filter_filters_cursor_position() {
        assert!(!CursorPositionFilter.eval(&InternalEvent::Event(Event::Resize(10, 10))));
        assert!(CursorPositionFilter.eval(&InternalEvent::CursorPosition(0, 0)));
    }

    #[test]
    fn test_keyboard_enhancement_status_filter_filters_keyboard_enhancement_status() {
        assert!(!KeyboardEnhancementFlagsFilter.eval(&InternalEvent::Event(Event::Resize(10, 10))));
        assert!(
            KeyboardEnhancementFlagsFilter.eval(&InternalEvent::KeyboardEnhancementFlags(
                crate::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            ))
        );
        assert!(KeyboardEnhancementFlagsFilter.eval(&InternalEvent::PrimaryDeviceAttributes));
    }

    #[test]
    fn test_primary_device_attributes_filter_filters_primary_device_attributes() {
        assert!(!PrimaryDeviceAttributesFilter.eval(&InternalEvent::Event(Event::Resize(10, 10))));
        assert!(PrimaryDeviceAttributesFilter.eval(&InternalEvent::PrimaryDeviceAttributes));
    }

    #[test]
    fn test_event_filter_filters_events() {
        assert!(EventFilter.eval(&InternalEvent::Event(Event::Resize(10, 10))));
        assert!(!EventFilter.eval(&InternalEvent::CursorPosition(0, 0)));
    }

    #[test]
    fn test_event_filter_filters_internal_events() {
        assert!(InternalEventFilter.eval(&InternalEvent::Event(Event::Resize(10, 10))));
        assert!(InternalEventFilter.eval(&InternalEvent::CursorPosition(0, 0)));
    }
}

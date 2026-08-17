#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]

pub(crate) mod base {
    pub use objc::runtime::{BOOL, NO, YES};

    pub type Class = *mut objc::runtime::Class;
    pub type SEL = objc::runtime::Sel;
    pub type id = *mut objc::runtime::Object;

    pub const nil: id = std::ptr::null_mut();
    pub const Nil: Class = std::ptr::null_mut();

    #[inline]
    pub fn selector(name: &str) -> SEL {
        objc::runtime::Sel::register(name)
    }
}

pub(crate) mod foundation {
    use super::base::{BOOL, NO, id, nil};
    use core_graphics::base::CGFloat;
    use core_graphics::geometry::CGRect;
    use objc::{class, msg_send, sel, sel_impl};
    use std::{ffi::CString, marker::PhantomData, mem, os::raw::c_char, ptr};

    #[cfg(target_pointer_width = "32")]
    pub type NSInteger = libc::c_int;
    #[cfg(target_pointer_width = "32")]
    pub type NSUInteger = libc::c_uint;

    #[cfg(target_pointer_width = "64")]
    pub type NSInteger = libc::c_long;
    #[cfg(target_pointer_width = "64")]
    pub type NSUInteger = libc::c_ulong;

    pub const NSIntegerMax: NSInteger = NSInteger::MAX;
    pub const NSNotFound: NSInteger = NSIntegerMax;
    const UTF8_ENCODING: NSUInteger = 4;

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    pub struct NSPoint {
        pub x: CGFloat,
        pub y: CGFloat,
    }

    impl NSPoint {
        #[inline]
        pub fn new(x: CGFloat, y: CGFloat) -> Self {
            Self { x, y }
        }
    }

    unsafe impl objc::Encode for NSPoint {
        fn encode() -> objc::Encoding {
            let encoding = format!(
                "{{CGPoint={}{}}}",
                CGFloat::encode().as_str(),
                CGFloat::encode().as_str()
            );
            unsafe { objc::Encoding::from_str(&encoding) }
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    pub struct NSSize {
        pub width: CGFloat,
        pub height: CGFloat,
    }

    impl NSSize {
        #[inline]
        pub fn new(width: CGFloat, height: CGFloat) -> Self {
            Self { width, height }
        }
    }

    unsafe impl objc::Encode for NSSize {
        fn encode() -> objc::Encoding {
            let encoding = format!(
                "{{CGSize={}{}}}",
                CGFloat::encode().as_str(),
                CGFloat::encode().as_str()
            );
            unsafe { objc::Encoding::from_str(&encoding) }
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    pub struct NSRect {
        pub origin: NSPoint,
        pub size: NSSize,
    }

    impl NSRect {
        #[inline]
        pub fn new(origin: NSPoint, size: NSSize) -> Self {
            Self { origin, size }
        }

        #[inline]
        pub fn as_CGRect(&self) -> &CGRect {
            unsafe { mem::transmute::<&NSRect, &CGRect>(self) }
        }

        #[inline]
        pub fn inset(&self, x: CGFloat, y: CGFloat) -> Self {
            unsafe { NSInsetRect(*self, x, y) }
        }
    }

    unsafe impl objc::Encode for NSRect {
        fn encode() -> objc::Encoding {
            let encoding = format!(
                "{{CGRect={}{}}}",
                NSPoint::encode().as_str(),
                NSSize::encode().as_str()
            );
            unsafe { objc::Encoding::from_str(&encoding) }
        }
    }

    #[repr(u32)]
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub enum NSRectEdge {
        NSRectMinXEdge,
        NSRectMinYEdge,
        NSRectMaxXEdge,
        NSRectMaxYEdge,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    pub struct NSRange {
        pub location: NSUInteger,
        pub length: NSUInteger,
    }

    impl NSRange {
        #[inline]
        pub fn new(location: NSUInteger, length: NSUInteger) -> Self {
            Self { location, length }
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default)]
    pub struct NSOperatingSystemVersion {
        pub majorVersion: NSUInteger,
        pub minorVersion: NSUInteger,
        pub patchVersion: NSUInteger,
    }

    impl NSOperatingSystemVersion {
        #[inline]
        pub fn new(
            majorVersion: NSUInteger,
            minorVersion: NSUInteger,
            patchVersion: NSUInteger,
        ) -> Self {
            Self {
                majorVersion,
                minorVersion,
                patchVersion,
            }
        }
    }

    unsafe impl objc::Encode for NSOperatingSystemVersion {
        fn encode() -> objc::Encoding {
            let encoding = format!(
                "{{?={}{}{}}}",
                NSUInteger::encode().as_str(),
                NSUInteger::encode().as_str(),
                NSUInteger::encode().as_str()
            );
            unsafe { objc::Encoding::from_str(&encoding) }
        }
    }

    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {
        fn NSInsetRect(rect: NSRect, x: CGFloat, y: CGFloat) -> NSRect;
    }

    pub trait NSAutoreleasePool: Sized {
        unsafe fn new(_: Self) -> id {
            unsafe { msg_send![class!(NSAutoreleasePool), new] }
        }

        unsafe fn autorelease(self) -> Self;
        unsafe fn drain(self);
    }

    impl NSAutoreleasePool for id {
        unsafe fn autorelease(self) -> id {
            unsafe { msg_send![self, autorelease] }
        }

        unsafe fn drain(self) {
            unsafe { msg_send![self, drain] }
        }
    }

    pub trait NSString: Sized {
        unsafe fn alloc(_: Self) -> id {
            unsafe { msg_send![class!(NSString), alloc] }
        }

        unsafe fn init_str(self, string: &str) -> id;
        unsafe fn UTF8String(self) -> *const c_char;
        unsafe fn lengthOfBytesUsingEncoding(self, encoding: NSUInteger) -> NSUInteger;
        unsafe fn isEqualToString(self, other: &str) -> bool;
    }

    impl NSString for id {
        unsafe fn init_str(self, string: &str) -> id {
            let string = CString::new(string).expect("NSString input contains interior NUL");
            unsafe {
                msg_send![
                    self,
                    initWithBytes: string.as_ptr()
                    length: string.as_bytes().len()
                    encoding: UTF8_ENCODING
                ]
            }
        }

        unsafe fn UTF8String(self) -> *const c_char {
            unsafe { msg_send![self, UTF8String] }
        }

        unsafe fn lengthOfBytesUsingEncoding(self, encoding: NSUInteger) -> NSUInteger {
            unsafe { msg_send![self, lengthOfBytesUsingEncoding: encoding] }
        }

        unsafe fn isEqualToString(self, other: &str) -> bool {
            let other = unsafe { NSString::alloc(nil).init_str(other).autorelease() };
            let equal: BOOL = unsafe { msg_send![self, isEqualToString: other] };
            equal != NO
        }
    }

    pub trait NSArray: Sized {
        unsafe fn array(_: Self) -> id {
            unsafe { msg_send![class!(NSArray), array] }
        }

        unsafe fn arrayWithObject(_: Self, object: id) -> id {
            unsafe { msg_send![class!(NSArray), arrayWithObject: object] }
        }

        unsafe fn arrayWithObjects(_: Self, objects: &[id]) -> id {
            unsafe {
                msg_send![
                    class!(NSArray),
                    arrayWithObjects: objects.as_ptr()
                    count: objects.len()
                ]
            }
        }

        unsafe fn count(self) -> NSUInteger;
        unsafe fn objectAtIndex(self, index: NSUInteger) -> id;
    }

    impl NSArray for id {
        unsafe fn count(self) -> NSUInteger {
            unsafe { msg_send![self, count] }
        }

        unsafe fn objectAtIndex(self, index: NSUInteger) -> id {
            unsafe { msg_send![self, objectAtIndex: index] }
        }
    }

    pub trait NSFastEnumeration: Sized {
        unsafe fn iter(self) -> NSArrayIter;
    }

    impl NSFastEnumeration for id {
        unsafe fn iter(self) -> NSArrayIter {
            let len = unsafe { NSArray::count(self) };
            NSArrayIter {
                array: self,
                index: 0,
                len,
                _not_send: PhantomData,
            }
        }
    }

    pub struct NSArrayIter {
        array: id,
        index: NSUInteger,
        len: NSUInteger,
        _not_send: PhantomData<*mut ()>,
    }

    impl Iterator for NSArrayIter {
        type Item = id;

        fn next(&mut self) -> Option<Self::Item> {
            if self.index >= self.len {
                return None;
            }
            let index = self.index;
            self.index += 1;
            Some(unsafe { NSArray::objectAtIndex(self.array, index) })
        }
    }

    pub trait NSDictionary: Sized {
        unsafe fn objectForKey_(self, key: id) -> id;
        unsafe fn valueForKey_(self, key: id) -> id;
    }

    impl NSDictionary for id {
        unsafe fn objectForKey_(self, key: id) -> id {
            unsafe { msg_send![self, objectForKey: key] }
        }

        unsafe fn valueForKey_(self, key: id) -> id {
            unsafe { msg_send![self, valueForKey: key] }
        }
    }

    pub trait NSData: Sized {
        unsafe fn dataWithBytes_length_(
            _: Self,
            bytes: *const std::ffi::c_void,
            length: u64,
        ) -> id {
            unsafe { msg_send![class!(NSData), dataWithBytes: bytes length: length] }
        }

        unsafe fn bytes(self) -> *const std::ffi::c_void;
        unsafe fn length(self) -> NSUInteger;
    }

    impl NSData for id {
        unsafe fn bytes(self) -> *const std::ffi::c_void {
            unsafe { msg_send![self, bytes] }
        }

        unsafe fn length(self) -> NSUInteger {
            unsafe { msg_send![self, length] }
        }
    }

    pub trait NSURL: Sized {
        unsafe fn alloc(_: Self) -> id {
            unsafe { msg_send![class!(NSURL), alloc] }
        }

        unsafe fn initWithString_(self, string: id) -> id;

        unsafe fn fileURLWithPath_(_: Self, path: id) -> id {
            unsafe { msg_send![class!(NSURL), fileURLWithPath: path] }
        }

        unsafe fn fileURLWithPath_isDirectory_(_: Self, path: id, is_directory: BOOL) -> id {
            unsafe { msg_send![class!(NSURL), fileURLWithPath: path isDirectory: is_directory] }
        }

        unsafe fn isFileURL(self) -> BOOL;
        unsafe fn absoluteString(self) -> id;
    }

    impl NSURL for id {
        unsafe fn initWithString_(self, string: id) -> id {
            unsafe { msg_send![self, initWithString: string] }
        }

        unsafe fn isFileURL(self) -> BOOL {
            unsafe { msg_send![self, isFileURL] }
        }

        unsafe fn absoluteString(self) -> id {
            unsafe { msg_send![self, absoluteString] }
        }
    }

    pub trait NSBundle: Sized {
        unsafe fn mainBundle() -> Self;
    }

    impl NSBundle for id {
        unsafe fn mainBundle() -> id {
            unsafe { msg_send![class!(NSBundle), mainBundle] }
        }
    }

    pub trait NSProcessInfo: Sized {
        unsafe fn processInfo(_: Self) -> id {
            unsafe { msg_send![class!(NSProcessInfo), processInfo] }
        }

        unsafe fn operatingSystemVersion(self) -> NSOperatingSystemVersion;
        unsafe fn isOperatingSystemAtLeastVersion(self, version: NSOperatingSystemVersion) -> bool;
    }

    impl NSProcessInfo for id {
        unsafe fn operatingSystemVersion(self) -> NSOperatingSystemVersion {
            unsafe { msg_send![self, operatingSystemVersion] }
        }

        unsafe fn isOperatingSystemAtLeastVersion(self, version: NSOperatingSystemVersion) -> bool {
            unsafe { msg_send![self, isOperatingSystemAtLeastVersion: version] }
        }
    }

    pub trait NSUserDefaults: Sized {
        unsafe fn standardUserDefaults() -> Self;
    }

    impl NSUserDefaults for id {
        unsafe fn standardUserDefaults() -> id {
            unsafe { msg_send![class!(NSUserDefaults), standardUserDefaults] }
        }
    }

    pub trait NSArrayObjectAccess: Sized {
        unsafe fn count(self) -> NSUInteger;
        unsafe fn objectAtIndex(self, index: NSUInteger) -> id;
    }

    impl NSArrayObjectAccess for id {
        unsafe fn count(self) -> NSUInteger {
            unsafe { NSArray::count(self) }
        }

        unsafe fn objectAtIndex(self, index: NSUInteger) -> id {
            unsafe { NSArray::objectAtIndex(self, index) }
        }
    }

    pub(crate) unsafe fn cf_release(object: id) {
        if !object.is_null() {
            unsafe {
                let _: () = msg_send![object, release];
            }
        }
    }

    pub(crate) fn null_id() -> id {
        ptr::null_mut()
    }
}

pub(crate) mod appkit {
    use super::{
        base::{BOOL, SEL, id},
        foundation::{NSInteger, NSPoint, NSRect, NSSize, NSUInteger},
    };
    use bitflags::bitflags;
    use objc::{class, msg_send, sel, sel_impl};

    pub use core_graphics::base::CGFloat;
    pub type CGLContextObj = *mut std::ffi::c_void;

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        pub static NSAppKitVersionNumber: f64;
        pub static NSFilenamesPboardType: id;
        pub static NSPasteboardTypeString: id;
        pub static NSPasteboardTypeTIFF: id;
        pub static NSPasteboardTypePNG: id;
        pub static NSAppearanceNameVibrantDark: id;
        pub static NSAppearanceNameVibrantLight: id;
    }

    pub const NSAppKitVersionNumber12_0: f64 = 2113.0;
    pub const NSBackingStoreBuffered: NSBackingStoreType =
        NSBackingStoreType::NSBackingStoreBuffered;
    pub const NSViewWidthSizable: NSUInteger = 1 << 1;
    pub const NSViewHeightSizable: NSUInteger = 1 << 4;

    #[repr(i64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSApplicationActivationPolicy {
        NSApplicationActivationPolicyRegular = 0,
        NSApplicationActivationPolicyAccessory = 1,
        NSApplicationActivationPolicyProhibited = 2,
        NSApplicationActivationPolicyERROR = -1,
    }

    #[repr(u64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSBackingStoreType {
        NSBackingStoreRetained = 0,
        NSBackingStoreNonretained = 1,
        NSBackingStoreBuffered = 2,
    }

    bitflags! {
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct NSWindowStyleMask: NSUInteger {
            const NSBorderlessWindowMask = 0;
            const NSTitledWindowMask = 1 << 0;
            const NSClosableWindowMask = 1 << 1;
            const NSMiniaturizableWindowMask = 1 << 2;
            const NSResizableWindowMask = 1 << 3;
            const NSTexturedBackgroundWindowMask = 1 << 8;
            const NSUnifiedTitleAndToolbarWindowMask = 1 << 12;
            const NSFullScreenWindowMask = 1 << 14;
            const NSFullSizeContentViewWindowMask = 1 << 15;
        }
    }

    #[repr(u64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSWindowTitleVisibility {
        NSWindowTitleVisible = 0,
        NSWindowTitleHidden = 1,
    }

    #[repr(i64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSWindowTabbingMode {
        NSWindowTabbingModeAutomatic = 0,
        NSWindowTabbingModePreferred = 1,
        NSWindowTabbingModeDisallowed = 2,
    }

    bitflags! {
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct NSWindowOrderingMode: NSInteger {
            const NSWindowAbove = 1;
            const NSWindowBelow = -1;
            const NSWindowOut = 0;
        }
    }

    bitflags! {
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct NSWindowCollectionBehavior: NSUInteger {
            const NSWindowCollectionBehaviorDefault = 0;
            const NSWindowCollectionBehaviorCanJoinAllSpaces = 1 << 0;
            const NSWindowCollectionBehaviorMoveToActiveSpace = 1 << 1;
            const NSWindowCollectionBehaviorManaged = 1 << 2;
            const NSWindowCollectionBehaviorTransient = 1 << 3;
            const NSWindowCollectionBehaviorStationary = 1 << 4;
            const NSWindowCollectionBehaviorParticipatesInCycle = 1 << 5;
            const NSWindowCollectionBehaviorIgnoresCycle = 1 << 6;
            const NSWindowCollectionBehaviorFullScreenPrimary = 1 << 7;
            const NSWindowCollectionBehaviorFullScreenAuxiliary = 1 << 8;
            const NSWindowCollectionBehaviorFullScreenNone = 1 << 9;
            const NSWindowCollectionBehaviorFullScreenAllowsTiling = 1 << 11;
            const NSWindowCollectionBehaviorFullScreenDisallowsTiling = 1 << 12;
        }
    }

    bitflags! {
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct NSWindowOcclusionState: NSUInteger {
            const NSWindowOcclusionStateVisible = 1 << 1;
        }
    }

    #[repr(u64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSWindowButton {
        NSWindowCloseButton = 0,
        NSWindowMiniaturizeButton = 1,
        NSWindowZoomButton = 2,
    }

    #[repr(u64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSVisualEffectBlendingMode {
        BehindWindow = 0,
        WithinWindow = 1,
    }

    #[repr(u64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSVisualEffectState {
        FollowsWindowActiveState = 0,
        Active = 1,
        Inactive = 2,
    }

    #[repr(u64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSVisualEffectMaterial {
        AppearanceBased = 0,
        Light = 1,
        Dark = 2,
        Titlebar = 3,
        Selection = 4,
        Menu = 5,
        Popover = 6,
        Sidebar = 7,
        MediumLight = 8,
        UltraDark = 9,
        HeaderView = 10,
        Sheet = 11,
        WindowBackground = 12,
        HudWindow = 13,
        FullScreenUI = 15,
        Tooltip = 17,
        ContentBackground = 18,
        UnderWindowBackground = 21,
        UnderPageBackground = 22,
    }

    #[repr(i64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSModalResponse {
        NSModalResponseOk = 1,
        NSModalResponseCancel = 0,
    }

    #[repr(u64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSEventType {
        NSLeftMouseDown = 1,
        NSLeftMouseUp = 2,
        NSRightMouseDown = 3,
        NSRightMouseUp = 4,
        NSMouseMoved = 5,
        NSLeftMouseDragged = 6,
        NSRightMouseDragged = 7,
        NSMouseEntered = 8,
        NSMouseExited = 9,
        NSKeyDown = 10,
        NSKeyUp = 11,
        NSFlagsChanged = 12,
        NSAppKitDefined = 13,
        NSSystemDefined = 14,
        NSApplicationDefined = 15,
        NSPeriodic = 16,
        NSCursorUpdate = 17,
        NSScrollWheel = 22,
        NSTabletPoint = 23,
        NSTabletProximity = 24,
        NSOtherMouseDown = 25,
        NSOtherMouseUp = 26,
        NSOtherMouseDragged = 27,
        NSEventTypeGesture = 29,
        NSEventTypeMagnify = 30,
        NSEventTypeSwipe = 31,
        NSEventTypeRotate = 18,
        NSEventTypeBeginGesture = 19,
        NSEventTypeEndGesture = 20,
        NSEventTypePressure = 34,
    }

    #[repr(u64)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum NSEventPhase {
        NSEventPhaseNone = 0,
        NSEventPhaseBegan = 0x1,
        NSEventPhaseStationary = 0x2,
        NSEventPhaseChanged = 0x4,
        NSEventPhaseEnded = 0x8,
        NSEventPhaseCancelled = 0x10,
        NSEventPhaseMayBegin = 0x20,
    }

    bitflags! {
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct NSEventModifierFlags: NSUInteger {
            const NSAlphaShiftKeyMask = 1 << 16;
            const NSShiftKeyMask = 1 << 17;
            const NSControlKeyMask = 1 << 18;
            const NSAlternateKeyMask = 1 << 19;
            const NSCommandKeyMask = 1 << 20;
            const NSNumericPadKeyMask = 1 << 21;
            const NSHelpKeyMask = 1 << 22;
            const NSFunctionKeyMask = 1 << 23;
            const NSDeviceIndependentModifierFlagsMask = 0xffff0000;
        }
    }

    pub const NSUpArrowFunctionKey: libc::c_ushort = 0xF700;
    pub const NSDownArrowFunctionKey: libc::c_ushort = 0xF701;
    pub const NSLeftArrowFunctionKey: libc::c_ushort = 0xF702;
    pub const NSRightArrowFunctionKey: libc::c_ushort = 0xF703;
    pub const NSF1FunctionKey: libc::c_ushort = 0xF704;
    pub const NSF2FunctionKey: libc::c_ushort = 0xF705;
    pub const NSF3FunctionKey: libc::c_ushort = 0xF706;
    pub const NSF4FunctionKey: libc::c_ushort = 0xF707;
    pub const NSF5FunctionKey: libc::c_ushort = 0xF708;
    pub const NSF6FunctionKey: libc::c_ushort = 0xF709;
    pub const NSF7FunctionKey: libc::c_ushort = 0xF70A;
    pub const NSF8FunctionKey: libc::c_ushort = 0xF70B;
    pub const NSF9FunctionKey: libc::c_ushort = 0xF70C;
    pub const NSF10FunctionKey: libc::c_ushort = 0xF70D;
    pub const NSF11FunctionKey: libc::c_ushort = 0xF70E;
    pub const NSF12FunctionKey: libc::c_ushort = 0xF70F;
    pub const NSF13FunctionKey: libc::c_ushort = 0xF710;
    pub const NSF14FunctionKey: libc::c_ushort = 0xF711;
    pub const NSF15FunctionKey: libc::c_ushort = 0xF712;
    pub const NSF16FunctionKey: libc::c_ushort = 0xF713;
    pub const NSF17FunctionKey: libc::c_ushort = 0xF714;
    pub const NSF18FunctionKey: libc::c_ushort = 0xF715;
    pub const NSF19FunctionKey: libc::c_ushort = 0xF716;
    pub const NSF20FunctionKey: libc::c_ushort = 0xF717;
    pub const NSF21FunctionKey: libc::c_ushort = 0xF718;
    pub const NSF22FunctionKey: libc::c_ushort = 0xF719;
    pub const NSF23FunctionKey: libc::c_ushort = 0xF71A;
    pub const NSF24FunctionKey: libc::c_ushort = 0xF71B;
    pub const NSF25FunctionKey: libc::c_ushort = 0xF71C;
    pub const NSF26FunctionKey: libc::c_ushort = 0xF71D;
    pub const NSF27FunctionKey: libc::c_ushort = 0xF71E;
    pub const NSF28FunctionKey: libc::c_ushort = 0xF71F;
    pub const NSF29FunctionKey: libc::c_ushort = 0xF720;
    pub const NSF30FunctionKey: libc::c_ushort = 0xF721;
    pub const NSF31FunctionKey: libc::c_ushort = 0xF722;
    pub const NSF32FunctionKey: libc::c_ushort = 0xF723;
    pub const NSF33FunctionKey: libc::c_ushort = 0xF724;
    pub const NSF34FunctionKey: libc::c_ushort = 0xF725;
    pub const NSF35FunctionKey: libc::c_ushort = 0xF726;
    pub const NSInsertFunctionKey: libc::c_ushort = 0xF727;
    pub const NSDeleteFunctionKey: libc::c_ushort = 0xF728;
    pub const NSHomeFunctionKey: libc::c_ushort = 0xF729;
    pub const NSBeginFunctionKey: libc::c_ushort = 0xF72A;
    pub const NSEndFunctionKey: libc::c_ushort = 0xF72B;
    pub const NSPageUpFunctionKey: libc::c_ushort = 0xF72C;
    pub const NSPageDownFunctionKey: libc::c_ushort = 0xF72D;
    pub const NSPrintScreenFunctionKey: libc::c_ushort = 0xF72E;
    pub const NSScrollLockFunctionKey: libc::c_ushort = 0xF72F;
    pub const NSPauseFunctionKey: libc::c_ushort = 0xF730;
    pub const NSSysReqFunctionKey: libc::c_ushort = 0xF731;
    pub const NSBreakFunctionKey: libc::c_ushort = 0xF732;
    pub const NSResetFunctionKey: libc::c_ushort = 0xF733;
    pub const NSStopFunctionKey: libc::c_ushort = 0xF734;
    pub const NSMenuFunctionKey: libc::c_ushort = 0xF735;
    pub const NSUserFunctionKey: libc::c_ushort = 0xF736;
    pub const NSSystemFunctionKey: libc::c_ushort = 0xF737;
    pub const NSPrintFunctionKey: libc::c_ushort = 0xF738;
    pub const NSClearLineFunctionKey: libc::c_ushort = 0xF739;
    pub const NSClearDisplayFunctionKey: libc::c_ushort = 0xF73A;
    pub const NSInsertLineFunctionKey: libc::c_ushort = 0xF73B;
    pub const NSDeleteLineFunctionKey: libc::c_ushort = 0xF73C;
    pub const NSInsertCharFunctionKey: libc::c_ushort = 0xF73D;
    pub const NSDeleteCharFunctionKey: libc::c_ushort = 0xF73E;
    pub const NSPrevFunctionKey: libc::c_ushort = 0xF73F;
    pub const NSNextFunctionKey: libc::c_ushort = 0xF740;
    pub const NSSelectFunctionKey: libc::c_ushort = 0xF741;
    pub const NSExecuteFunctionKey: libc::c_ushort = 0xF742;
    pub const NSUndoFunctionKey: libc::c_ushort = 0xF743;
    pub const NSRedoFunctionKey: libc::c_ushort = 0xF744;
    pub const NSFindFunctionKey: libc::c_ushort = 0xF745;
    pub const NSHelpFunctionKey: libc::c_ushort = 0xF746;
    pub const NSModeSwitchFunctionKey: libc::c_ushort = 0xF747;

    pub trait NSApplication: Sized {
        unsafe fn sharedApplication(_: Self) -> id {
            unsafe { msg_send![class!(NSApplication), sharedApplication] }
        }

        unsafe fn run(self);
        unsafe fn delegate(self) -> id;
        unsafe fn setMainMenu_(self, menu: id);
        unsafe fn setWindowsMenu_(self, menu: id);
        unsafe fn setServicesMenu_(self, menu: id);
        unsafe fn activateIgnoringOtherApps_(self, ignoring_other_apps: BOOL);
        unsafe fn setActivationPolicy_(self, policy: NSApplicationActivationPolicy);
    }

    impl NSApplication for id {
        unsafe fn run(self) {
            unsafe { msg_send![self, run] }
        }

        unsafe fn delegate(self) -> id {
            unsafe { msg_send![self, delegate] }
        }

        unsafe fn setMainMenu_(self, menu: id) {
            unsafe { msg_send![self, setMainMenu: menu] }
        }

        unsafe fn setWindowsMenu_(self, menu: id) {
            unsafe { msg_send![self, setWindowsMenu: menu] }
        }

        unsafe fn setServicesMenu_(self, menu: id) {
            unsafe { msg_send![self, setServicesMenu: menu] }
        }

        unsafe fn activateIgnoringOtherApps_(self, ignoring_other_apps: BOOL) {
            unsafe { msg_send![self, activateIgnoringOtherApps: ignoring_other_apps] }
        }

        unsafe fn setActivationPolicy_(self, policy: NSApplicationActivationPolicy) {
            unsafe { msg_send![self, setActivationPolicy: policy] }
        }
    }

    pub trait NSMenu: Sized {
        unsafe fn new(_: Self) -> id {
            unsafe { msg_send![class!(NSMenu), new] }
        }

        unsafe fn addItem_(self, item: id);
    }

    impl NSMenu for id {
        unsafe fn addItem_(self, item: id) {
            unsafe { msg_send![self, addItem: item] }
        }
    }

    pub trait NSMenuItem: Sized {
        unsafe fn alloc(_: Self) -> id {
            unsafe { msg_send![class!(NSMenuItem), alloc] }
        }

        unsafe fn new(_: Self) -> id {
            unsafe { msg_send![class!(NSMenuItem), new] }
        }

        unsafe fn separatorItem(_: Self) -> id {
            unsafe { msg_send![class!(NSMenuItem), separatorItem] }
        }

        unsafe fn initWithTitle_action_keyEquivalent_(self, title: id, action: SEL, key: id) -> id;
        unsafe fn setKeyEquivalentModifierMask_(self, mask: NSEventModifierFlags);
        unsafe fn setSubmenu_(self, submenu: id);
    }

    impl NSMenuItem for id {
        unsafe fn initWithTitle_action_keyEquivalent_(self, title: id, action: SEL, key: id) -> id {
            unsafe { msg_send![self, initWithTitle: title action: action keyEquivalent: key] }
        }

        unsafe fn setKeyEquivalentModifierMask_(self, mask: NSEventModifierFlags) {
            unsafe { msg_send![self, setKeyEquivalentModifierMask: mask] }
        }

        unsafe fn setSubmenu_(self, submenu: id) {
            unsafe { msg_send![self, setSubmenu: submenu] }
        }
    }

    pub trait NSControl: Sized {
        unsafe fn setEnabled_(self, enabled: BOOL) -> BOOL;
    }

    impl NSControl for id {
        unsafe fn setEnabled_(self, enabled: BOOL) -> BOOL {
            unsafe { msg_send![self, setEnabled: enabled] }
        }
    }

    pub trait NSWindow: Sized {
        unsafe fn alloc(_: Self) -> id {
            unsafe { msg_send![class!(NSWindow), alloc] }
        }

        unsafe fn delegate(self) -> id;
        unsafe fn frame(self) -> NSRect;
        unsafe fn screen(self) -> id;
        unsafe fn contentView(self) -> id;
        unsafe fn styleMask(self) -> NSWindowStyleMask;
        unsafe fn isVisible(self) -> BOOL;
        unsafe fn isKeyWindow(self) -> BOOL;
        unsafe fn occlusionState(self) -> NSWindowOcclusionState;
        unsafe fn windowNumber(self) -> NSInteger;
        unsafe fn mouseLocationOutsideOfEventStream(self) -> NSPoint;
        unsafe fn standardWindowButton_(self, button: NSWindowButton) -> id;
        unsafe fn initWithContentRect_styleMask_backing_defer_screen_(
            self,
            rect: NSRect,
            style: NSWindowStyleMask,
            backing: NSBackingStoreType,
            defer: BOOL,
            screen: id,
        ) -> id;
        unsafe fn setDelegate_(self, delegate: id);
        unsafe fn setTitle_(self, title: id);
        unsafe fn close(self);
        unsafe fn setMovable_(self, movable: BOOL);
        unsafe fn setContentMinSize_(self, size: NSSize);
        unsafe fn setTitleVisibility_(self, visibility: NSWindowTitleVisibility);
        unsafe fn setTitlebarAppearsTransparent_(self, transparent: BOOL);
        unsafe fn makeFirstResponder_(self, responder: id);
        unsafe fn setLevel_(self, level: NSInteger);
        unsafe fn setAcceptsMouseMovedEvents_(self, accepts: BOOL);
        unsafe fn setCollectionBehavior_(self, behavior: NSWindowCollectionBehavior);
        unsafe fn setTabbingMode_(self, mode: NSWindowTabbingMode);
        unsafe fn makeKeyAndOrderFront_(self, sender: id);
        unsafe fn orderFront_(self, sender: id);
        unsafe fn setFrameTopLeftPoint_(self, point: NSPoint);
        unsafe fn setContentSize_(self, size: NSSize);
        unsafe fn setOpaque_(self, opaque: BOOL);
        unsafe fn setBackgroundColor_(self, color: id);
        unsafe fn miniaturize_(self, sender: id);
        unsafe fn zoom_(self, sender: id);
        unsafe fn toggleFullScreen_(self, sender: id);
    }

    impl NSWindow for id {
        unsafe fn delegate(self) -> id {
            unsafe { msg_send![self, delegate] }
        }

        unsafe fn frame(self) -> NSRect {
            unsafe { msg_send![self, frame] }
        }

        unsafe fn screen(self) -> id {
            unsafe { msg_send![self, screen] }
        }

        unsafe fn contentView(self) -> id {
            unsafe { msg_send![self, contentView] }
        }

        unsafe fn styleMask(self) -> NSWindowStyleMask {
            NSWindowStyleMask::from_bits_truncate(unsafe { msg_send![self, styleMask] })
        }

        unsafe fn isVisible(self) -> BOOL {
            unsafe { msg_send![self, isVisible] }
        }

        unsafe fn isKeyWindow(self) -> BOOL {
            unsafe { msg_send![self, isKeyWindow] }
        }

        unsafe fn occlusionState(self) -> NSWindowOcclusionState {
            NSWindowOcclusionState::from_bits_truncate(unsafe { msg_send![self, occlusionState] })
        }

        unsafe fn windowNumber(self) -> NSInteger {
            unsafe { msg_send![self, windowNumber] }
        }

        unsafe fn mouseLocationOutsideOfEventStream(self) -> NSPoint {
            unsafe { msg_send![self, mouseLocationOutsideOfEventStream] }
        }

        unsafe fn standardWindowButton_(self, button: NSWindowButton) -> id {
            unsafe { msg_send![self, standardWindowButton: button] }
        }

        unsafe fn initWithContentRect_styleMask_backing_defer_screen_(
            self,
            rect: NSRect,
            style: NSWindowStyleMask,
            backing: NSBackingStoreType,
            defer: BOOL,
            screen: id,
        ) -> id {
            unsafe {
                msg_send![
                    self,
                    initWithContentRect: rect
                    styleMask: style.bits()
                    backing: backing as NSUInteger
                    defer: defer
                    screen: screen
                ]
            }
        }

        unsafe fn setDelegate_(self, delegate: id) {
            unsafe { msg_send![self, setDelegate: delegate] }
        }

        unsafe fn setTitle_(self, title: id) {
            unsafe { msg_send![self, setTitle: title] }
        }

        unsafe fn close(self) {
            unsafe { msg_send![self, close] }
        }

        unsafe fn setMovable_(self, movable: BOOL) {
            unsafe { msg_send![self, setMovable: movable] }
        }

        unsafe fn setContentMinSize_(self, size: NSSize) {
            unsafe { msg_send![self, setContentMinSize: size] }
        }

        unsafe fn setTitleVisibility_(self, visibility: NSWindowTitleVisibility) {
            unsafe { msg_send![self, setTitleVisibility: visibility] }
        }

        unsafe fn setTitlebarAppearsTransparent_(self, transparent: BOOL) {
            unsafe { msg_send![self, setTitlebarAppearsTransparent: transparent] }
        }

        unsafe fn makeFirstResponder_(self, responder: id) {
            unsafe { msg_send![self, makeFirstResponder: responder] }
        }

        unsafe fn setLevel_(self, level: NSInteger) {
            unsafe { msg_send![self, setLevel: level] }
        }

        unsafe fn setAcceptsMouseMovedEvents_(self, accepts: BOOL) {
            unsafe { msg_send![self, setAcceptsMouseMovedEvents: accepts] }
        }

        unsafe fn setCollectionBehavior_(self, behavior: NSWindowCollectionBehavior) {
            unsafe { msg_send![self, setCollectionBehavior: behavior] }
        }

        unsafe fn setTabbingMode_(self, mode: NSWindowTabbingMode) {
            unsafe { msg_send![self, setTabbingMode: mode as NSInteger] }
        }

        unsafe fn makeKeyAndOrderFront_(self, sender: id) {
            unsafe { msg_send![self, makeKeyAndOrderFront: sender] }
        }

        unsafe fn orderFront_(self, sender: id) {
            unsafe { msg_send![self, orderFront: sender] }
        }

        unsafe fn setFrameTopLeftPoint_(self, point: NSPoint) {
            unsafe { msg_send![self, setFrameTopLeftPoint: point] }
        }

        unsafe fn setContentSize_(self, size: NSSize) {
            unsafe { msg_send![self, setContentSize: size] }
        }

        unsafe fn setOpaque_(self, opaque: BOOL) {
            unsafe { msg_send![self, setOpaque: opaque] }
        }

        unsafe fn setBackgroundColor_(self, color: id) {
            unsafe { msg_send![self, setBackgroundColor: color] }
        }

        unsafe fn miniaturize_(self, sender: id) {
            unsafe { msg_send![self, miniaturize: sender] }
        }

        unsafe fn zoom_(self, sender: id) {
            unsafe { msg_send![self, zoom: sender] }
        }

        unsafe fn toggleFullScreen_(self, sender: id) {
            unsafe { msg_send![self, toggleFullScreen: sender] }
        }
    }

    pub trait NSView: Sized {
        unsafe fn alloc(_: Self) -> id {
            unsafe { msg_send![class!(NSView), alloc] }
        }

        unsafe fn initWithFrame_(self, frame: NSRect) -> id;
        unsafe fn bounds(self) -> NSRect;
        unsafe fn frame(self) -> NSRect;
        unsafe fn setAutoresizingMask_(self, mask: NSUInteger);
        unsafe fn setWantsBestResolutionOpenGLSurface_(self, flag: BOOL);
        unsafe fn addSubview_(self, view: id);
        unsafe fn setWantsLayer(self, wants_layer: BOOL);
        unsafe fn removeFromSuperview(self);
    }

    impl NSView for id {
        unsafe fn initWithFrame_(self, frame: NSRect) -> id {
            unsafe { msg_send![self, initWithFrame: frame] }
        }

        unsafe fn bounds(self) -> NSRect {
            unsafe { msg_send![self, bounds] }
        }

        unsafe fn frame(self) -> NSRect {
            unsafe { msg_send![self, frame] }
        }

        unsafe fn setAutoresizingMask_(self, mask: NSUInteger) {
            unsafe { msg_send![self, setAutoresizingMask: mask] }
        }

        unsafe fn setWantsBestResolutionOpenGLSurface_(self, flag: BOOL) {
            unsafe { msg_send![self, setWantsBestResolutionOpenGLSurface: flag] }
        }

        unsafe fn addSubview_(self, view: id) {
            unsafe { msg_send![self, addSubview: view] }
        }

        unsafe fn setWantsLayer(self, wants_layer: BOOL) {
            unsafe { msg_send![self, setWantsLayer: wants_layer] }
        }

        unsafe fn removeFromSuperview(self) {
            unsafe { msg_send![self, removeFromSuperview] }
        }
    }

    pub trait NSVisualEffectView: Sized {
        unsafe fn alloc(_: Self) -> id {
            unsafe { msg_send![class!(NSVisualEffectView), alloc] }
        }

        unsafe fn initWithFrame_(self, frame: NSRect) -> id;
        unsafe fn setMaterial_(self, material: NSVisualEffectMaterial);
        unsafe fn setState_(self, state: NSVisualEffectState);
        unsafe fn setBlendingMode_(self, mode: NSVisualEffectBlendingMode);
    }

    impl NSVisualEffectView for id {
        unsafe fn initWithFrame_(self, frame: NSRect) -> id {
            unsafe { msg_send![self, initWithFrame: frame] }
        }

        unsafe fn setMaterial_(self, material: NSVisualEffectMaterial) {
            unsafe { msg_send![self, setMaterial: material] }
        }

        unsafe fn setState_(self, state: NSVisualEffectState) {
            unsafe { msg_send![self, setState: state] }
        }

        unsafe fn setBlendingMode_(self, mode: NSVisualEffectBlendingMode) {
            unsafe { msg_send![self, setBlendingMode: mode] }
        }
    }

    pub trait NSScreen: Sized {
        unsafe fn screens(_: Self) -> id {
            unsafe { msg_send![class!(NSScreen), screens] }
        }

        unsafe fn mainScreen(_: Self) -> id {
            unsafe { msg_send![class!(NSScreen), mainScreen] }
        }

        unsafe fn frame(self) -> NSRect;
        unsafe fn visibleFrame(self) -> NSRect;
        unsafe fn deviceDescription(self) -> id;
        unsafe fn backingScaleFactor(self) -> CGFloat;
    }

    impl NSScreen for id {
        unsafe fn frame(self) -> NSRect {
            unsafe { msg_send![self, frame] }
        }

        unsafe fn visibleFrame(self) -> NSRect {
            unsafe { msg_send![self, visibleFrame] }
        }

        unsafe fn deviceDescription(self) -> id {
            unsafe { msg_send![self, deviceDescription] }
        }

        unsafe fn backingScaleFactor(self) -> CGFloat {
            unsafe { msg_send![self, backingScaleFactor] }
        }
    }

    pub trait NSPasteboard: Sized {
        unsafe fn generalPasteboard(_: Self) -> id {
            unsafe { msg_send![class!(NSPasteboard), generalPasteboard] }
        }

        unsafe fn pasteboardWithName(_: Self, name: id) -> id {
            unsafe { msg_send![class!(NSPasteboard), pasteboardWithName: name] }
        }

        unsafe fn pasteboardWithUniqueName(_: Self) -> id {
            unsafe { msg_send![class!(NSPasteboard), pasteboardWithUniqueName] }
        }

        unsafe fn propertyListForType(self, pasteboard_type: id) -> id;
        unsafe fn types(self) -> id;
        unsafe fn dataForType(self, pasteboard_type: id) -> id;
        unsafe fn clearContents(self);
        unsafe fn declareTypes_owner(self, types: id, owner: id) -> NSInteger;
        unsafe fn setData_forType(self, data: id, pasteboard_type: id) -> BOOL;
        unsafe fn setPropertyList_forType(self, property_list: id, pasteboard_type: id) -> BOOL;
    }

    impl NSPasteboard for id {
        unsafe fn propertyListForType(self, pasteboard_type: id) -> id {
            unsafe { msg_send![self, propertyListForType: pasteboard_type] }
        }

        unsafe fn types(self) -> id {
            unsafe { msg_send![self, types] }
        }

        unsafe fn dataForType(self, pasteboard_type: id) -> id {
            unsafe { msg_send![self, dataForType: pasteboard_type] }
        }

        unsafe fn clearContents(self) {
            unsafe { msg_send![self, clearContents] }
        }

        unsafe fn declareTypes_owner(self, types: id, owner: id) -> NSInteger {
            unsafe { msg_send![self, declareTypes: types owner: owner] }
        }

        unsafe fn setData_forType(self, data: id, pasteboard_type: id) -> BOOL {
            unsafe { msg_send![self, setData: data forType: pasteboard_type] }
        }

        unsafe fn setPropertyList_forType(self, property_list: id, pasteboard_type: id) -> BOOL {
            unsafe { msg_send![self, setPropertyList: property_list forType: pasteboard_type] }
        }
    }

    pub trait NSColor: Sized {
        unsafe fn colorWithSRGBRed_green_blue_alpha_(
            _: Self,
            red: f64,
            green: f64,
            blue: f64,
            alpha: f64,
        ) -> id {
            unsafe {
                msg_send![
                    class!(NSColor),
                    colorWithSRGBRed: red
                    green: green
                    blue: blue
                    alpha: alpha
                ]
            }
        }
    }

    impl NSColor for id {}

    pub trait NSSavePanel: Sized {
        unsafe fn savePanel(_: Self) -> id {
            unsafe { msg_send![class!(NSSavePanel), savePanel] }
        }

        unsafe fn setDirectoryURL(self, url: id);
        unsafe fn setCanCreateDirectories(self, can_create_directories: BOOL);
        unsafe fn URL(self) -> id;
    }

    impl NSSavePanel for id {
        unsafe fn setDirectoryURL(self, url: id) {
            unsafe { msg_send![self, setDirectoryURL: url] }
        }

        unsafe fn setCanCreateDirectories(self, can_create_directories: BOOL) {
            unsafe { msg_send![self, setCanCreateDirectories: can_create_directories] }
        }

        unsafe fn URL(self) -> id {
            unsafe { msg_send![self, URL] }
        }
    }

    pub trait NSOpenPanel: NSSavePanel {
        unsafe fn openPanel(_: Self) -> id {
            unsafe { msg_send![class!(NSOpenPanel), openPanel] }
        }

        unsafe fn setCanChooseFiles_(self, can_choose_files: BOOL);
        unsafe fn setCanChooseDirectories_(self, can_choose_directories: BOOL);
        unsafe fn setResolvesAliases_(self, resolves_aliases: BOOL);
        unsafe fn setAllowsMultipleSelection_(self, allows_multiple_selection: BOOL);
        unsafe fn URLs(self) -> id;
    }

    impl NSOpenPanel for id {
        unsafe fn setCanChooseFiles_(self, can_choose_files: BOOL) {
            unsafe { msg_send![self, setCanChooseFiles: can_choose_files] }
        }

        unsafe fn setCanChooseDirectories_(self, can_choose_directories: BOOL) {
            unsafe { msg_send![self, setCanChooseDirectories: can_choose_directories] }
        }

        unsafe fn setResolvesAliases_(self, resolves_aliases: BOOL) {
            unsafe { msg_send![self, setResolvesAliases: resolves_aliases] }
        }

        unsafe fn setAllowsMultipleSelection_(self, allows_multiple_selection: BOOL) {
            unsafe { msg_send![self, setAllowsMultipleSelection: allows_multiple_selection] }
        }

        unsafe fn URLs(self) -> id {
            unsafe { msg_send![self, URLs] }
        }
    }

    pub trait NSEvent: Sized {
        unsafe fn mouseLocation(_: Self) -> NSPoint {
            unsafe { msg_send![class!(NSEvent), mouseLocation] }
        }

        unsafe fn eventType(self) -> NSEventType;
        unsafe fn modifierFlags(self) -> NSEventModifierFlags;
        unsafe fn isARepeat(self) -> BOOL;
        unsafe fn keyCode(self) -> u16;
        unsafe fn characters(self) -> id;
        unsafe fn charactersIgnoringModifiers(self) -> id;
        unsafe fn locationInWindow(self) -> NSPoint;
        unsafe fn buttonNumber(self) -> NSInteger;
        unsafe fn clickCount(self) -> NSInteger;
        unsafe fn pressure(self) -> f32;
        unsafe fn stage(self) -> NSInteger;
        unsafe fn phase(self) -> NSEventPhase;
        unsafe fn momentumPhase(self) -> NSEventPhase;
        unsafe fn deltaX(self) -> f64;
        unsafe fn deltaY(self) -> f64;
        unsafe fn scrollingDeltaX(self) -> f64;
        unsafe fn scrollingDeltaY(self) -> f64;
        unsafe fn hasPreciseScrollingDeltas(self) -> BOOL;
        unsafe fn magnification(self) -> f64;
    }

    impl NSEvent for id {
        unsafe fn eventType(self) -> NSEventType {
            unsafe { msg_send![self, type] }
        }

        unsafe fn modifierFlags(self) -> NSEventModifierFlags {
            NSEventModifierFlags::from_bits_truncate(unsafe { msg_send![self, modifierFlags] })
        }

        unsafe fn isARepeat(self) -> BOOL {
            unsafe { msg_send![self, isARepeat] }
        }

        unsafe fn keyCode(self) -> u16 {
            unsafe { msg_send![self, keyCode] }
        }

        unsafe fn characters(self) -> id {
            unsafe { msg_send![self, characters] }
        }

        unsafe fn charactersIgnoringModifiers(self) -> id {
            unsafe { msg_send![self, charactersIgnoringModifiers] }
        }

        unsafe fn locationInWindow(self) -> NSPoint {
            unsafe { msg_send![self, locationInWindow] }
        }

        unsafe fn buttonNumber(self) -> NSInteger {
            unsafe { msg_send![self, buttonNumber] }
        }

        unsafe fn clickCount(self) -> NSInteger {
            unsafe { msg_send![self, clickCount] }
        }

        unsafe fn pressure(self) -> f32 {
            unsafe { msg_send![self, pressure] }
        }

        unsafe fn stage(self) -> NSInteger {
            unsafe { msg_send![self, stage] }
        }

        unsafe fn phase(self) -> NSEventPhase {
            unsafe { msg_send![self, phase] }
        }

        unsafe fn momentumPhase(self) -> NSEventPhase {
            unsafe { msg_send![self, momentumPhase] }
        }

        unsafe fn deltaX(self) -> f64 {
            unsafe { msg_send![self, deltaX] }
        }

        unsafe fn deltaY(self) -> f64 {
            unsafe { msg_send![self, deltaY] }
        }

        unsafe fn scrollingDeltaX(self) -> f64 {
            unsafe { msg_send![self, scrollingDeltaX] }
        }

        unsafe fn scrollingDeltaY(self) -> f64 {
            unsafe { msg_send![self, scrollingDeltaY] }
        }

        unsafe fn hasPreciseScrollingDeltas(self) -> BOOL {
            unsafe { msg_send![self, hasPreciseScrollingDeltas] }
        }

        unsafe fn magnification(self) -> f64 {
            unsafe { msg_send![self, magnification] }
        }
    }

    pub trait NSResponder {}
}

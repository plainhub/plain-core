
    use crate::rc::Retained;
    use crate::runtime::NSObject;

    #[test]
    fn retain_release_30_times() {
        #[inline(never)]
        fn new_obj() -> Retained<NSObject> {
            NSObject::new()
        }

        let obj00 = new_obj();
        let obj01 = new_obj();
        let obj02 = new_obj();
        let obj03 = new_obj();
        let obj04 = new_obj();
        let obj05 = new_obj();
        let obj06 = new_obj();
        let obj07 = new_obj();
        let obj08 = new_obj();
        let obj09 = new_obj();
        let obj10 = new_obj();
        let obj11 = new_obj();
        let obj12 = new_obj();
        let obj13 = new_obj();
        let obj14 = new_obj();
        let obj15 = new_obj();
        let obj16 = new_obj();
        let obj17 = new_obj();
        let obj18 = new_obj();
        let obj19 = new_obj();
        let obj20 = new_obj();
        let obj21 = new_obj();
        let obj22 = new_obj();
        let obj23 = new_obj();
        let obj24 = new_obj();
        let obj25 = new_obj();
        let obj26 = new_obj();
        let obj27 = new_obj();
        let obj28 = new_obj();
        let obj29 = new_obj();
        let obj30 = new_obj();

        let _obj00 = obj00.clone();
        let _obj01 = obj01.clone();
        let _obj02 = obj02.clone();
        let _obj03 = obj03.clone();
        let _obj04 = obj04.clone();
        let _obj05 = obj05.clone();
        let _obj06 = obj06.clone();
        let _obj07 = obj07.clone();
        let _obj08 = obj08.clone();
        let _obj09 = obj09.clone();
        let _obj10 = obj10.clone();
        let _obj11 = obj11.clone();
        let _obj12 = obj12.clone();
        let _obj13 = obj13.clone();
        let _obj14 = obj14.clone();
        let _obj15 = obj15.clone();
        let _obj16 = obj16.clone();
        let _obj17 = obj17.clone();
        let _obj18 = obj18.clone();
        let _obj19 = obj19.clone();
        let _obj20 = obj20.clone();
        let _obj21 = obj21.clone();
        let _obj22 = obj22.clone();
        let _obj23 = obj23.clone();
        let _obj24 = obj24.clone();
        let _obj25 = obj25.clone();
        let _obj26 = obj26.clone();
        let _obj27 = obj27.clone();
        let _obj28 = obj28.clone();
        let _obj29 = obj29.clone();
        let _obj30 = obj30.clone();
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn move_to_register_x16() {
        let obj = NSObject::new();
        let mut ptr = Retained::into_raw(obj);
        unsafe { core::arch::asm!("nop", inout("x16") ptr) };
        let _ = unsafe { Retained::from_raw(ptr) };

        let obj = NSObject::new();
        let mut ptr = Retained::autorelease_ptr(obj);
        unsafe { core::arch::asm!("nop", inout("x16") ptr) };
        let _ = unsafe { Retained::retain(ptr) };
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn move_to_register_x17() {
        let obj = NSObject::new();
        let mut ptr = Retained::into_raw(obj);
        unsafe { core::arch::asm!("nop", inout("x17") ptr) };
        let _ = unsafe { Retained::from_raw(ptr) };

        let obj = NSObject::new();
        let mut ptr = Retained::autorelease_ptr(obj);
        unsafe { core::arch::asm!("nop", inout("x17") ptr) };
        let _ = unsafe { Retained::retain(ptr) };
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn move_to_register_x30() {
        let obj = NSObject::new();
        let mut ptr = Retained::into_raw(obj);
        unsafe { core::arch::asm!("nop", inout("x30") ptr) };
        let _ = unsafe { Retained::from_raw(ptr) };

        let obj = NSObject::new();
        let mut ptr = Retained::autorelease_ptr(obj);
        unsafe { core::arch::asm!("nop", inout("x30") ptr) };
        let _ = unsafe { Retained::retain(ptr) };
    }


#![allow(dead_code)]
#![feature(const_fn)]
#![feature(unsize)]
#![feature(generator_trait)]
#![feature(generators)]
#![feature(never_type)]

use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ops;
use core::ptr;
use core::marker::Unsize;
use core::fmt;

pub type DynBoxS0<T> = DynBox<T, [usize;0]>;
pub type DynBoxS1<T> = DynBox<T, [usize;1]>;
pub type DynBoxS2<T> = DynBox<T, [usize;2]>;
pub type DynBoxS3<T> = DynBox<T, [usize;3]>;
pub type DynBoxS4<T> = DynBox<T, [usize;4]>;
pub type DynBoxS6<T> = DynBox<T, [usize;6]>;
pub type DynBoxS8<T> = DynBox<T, [usize;8]>;
pub type DynBoxS16<T> = DynBox<T, [usize;16]>;

pub struct DynBox<T, Space> 
where 
      T : ?Sized,
{
    space: ManuallyDrop<Space>,
    ptr : Option<*mut T>,
    _phantom: PhantomData<T>,
}


impl<T, Space> DynBox<T, Space> 
where 
      T : ?Sized,
{
    #[inline(always)]
    pub const fn empty(space : Space) -> DynBox<T, Space> {
        DynBox {
            space : ManuallyDrop::new(space),
            ptr : None,
            _phantom : PhantomData,
        }
    }

    #[inline(always)]
    pub fn occupy<U>(&mut self, val: U) -> ()
    where
        U: Sized + Unsize<T>,
    {
        unsafe { self.inner_new(&val) };
        mem::forget(val);
    }

    #[inline(always)]
    pub fn new<U>(val: U, space : Space) -> DynBox<T, Space>
    where
        U: Sized + Unsize<T>
    {
        let mut result = Self::empty(space);
        unsafe { result.inner_new(&val) };
        mem::forget(val);
        result
    }

    unsafe fn inner_new<U>(&mut self, val: &U)
    where
        U: Sized + Unsize<T>
    {
        let size = mem::size_of_val::<U>(val);
        let align = mem::align_of_val::<U>(val);

        let (ptr_src, ptr_dst): (*const u8, *mut u8) = if size == 0 { 
            // ZST
            (ptr::null(), align as *mut u8)
        } else if size <= mem::size_of::<Space>() && align <= mem::align_of::<Space>() {
            // Stack
            (val as *const _ as *const u8, mem::transmute(&mut self.space))
        } else {
            // Not enough space or aligment in Space
            panic!("Can not fit value into storage! storage size {} < value size {} OR storage align {} < value align {}",
                mem::size_of::<Space>(), size, mem::align_of::<Space>(), align);
        };

        ptr::copy_nonoverlapping(ptr_src, ptr_dst, size);

        let thin_ptr : *mut U = mem::transmute(&mut self.space);
        let fat_ptr : &mut T = &mut (*thin_ptr);

        self.ptr = Some(fat_ptr);
    }

    #[inline]
    unsafe fn as_ptr(&self) -> *mut T {
        let mut ptr = self.ptr.expect("DynBox is empty!");

        let ptr_ptr = &mut ptr as *mut _ as *mut usize;
        ptr_ptr.write(mem::transmute(&self.space));

        ptr        
    }
}

impl<T: ?Sized, Space> ops::Deref for DynBox<T, Space> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.as_ptr() }
    }
}

impl<T: ?Sized, Space> ops::DerefMut for DynBox<T, Space> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.as_ptr() }
    }
}

impl<T: ?Sized, Space : core::fmt::Debug> fmt::Debug for DynBox<T, Space> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        write!(f, "DynBox {{\n")?;
        write!(f, "    FatPtr:      ")?;
        match self.ptr.as_ref() {
            Some(ptr) => {
                //Max fat ptr size == usize * 2
                let ptr_view : &[usize; 2] = unsafe { mem::transmute(ptr) };
                write!(f, "{:x?}\n", ptr_view)?;
            }
            None => {
                write!(f, "None\n")?;
            }
        };
        write!(f, "    Space addr: {:?}\n", &self.space as *const _)?;
        let space : &Space = unsafe { mem::transmute(&self.space) };
        write!(f ,"    Space {:x?}\n", space)?;

        write!(f, "}}\n")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::ops::DerefMut;
    use core::ops::{Generator, GeneratorState};
    use core::pin::Pin;

    #[should_panic(expected = "DynBox is empty!")]
    #[test]
    fn test_panic() {
        static mut PTR : DynBox<dyn Fn(), [usize;1]> = DynBox::empty([0usize;1]);
        unsafe { 
            PTR();
        };

    }

    #[test]
    fn test_occupy_closure_stateless(){
        static mut PTR : DynBox<dyn Fn(), [usize;0]> = DynBox::empty([0usize;0]);
        static mut FLAG : bool = false;

        unsafe {
            PTR.occupy(|| {
                FLAG = true;
            });

            PTR();

            assert_eq!(FLAG, true);
        }
    }

    #[test]
    fn test_occupy_closure_statefull(){
        let mut ptr : DynBox<dyn FnMut(), [usize;2]> = DynBox::empty([0usize;2]);
        let mut flag : bool = false;
        let mut cnt = 0;

        ptr.occupy(|| {
            flag = true;
            cnt += 1;
        });

        ptr.deref_mut()();
        ptr.deref_mut()();
        ptr.deref_mut()();

        assert_eq!(flag, true);
        assert_eq!(cnt, 3);
    }

    #[test]
    fn test_moving_around() {
        fn outer(mut cb : DynBox<dyn FnMut() -> (bool, i32), [usize;2]>) -> DynBox<dyn FnMut()-> (bool, i32), [usize;2]>{
            cb.deref_mut()();
            inner(cb)
        }
        fn inner(mut cb : DynBox<dyn FnMut() -> (bool, i32), [usize;2]>) -> DynBox<dyn FnMut()-> (bool, i32), [usize;2]>{
            cb.deref_mut()();
            cb
        }

        let mut ptr : DynBox<dyn FnMut() -> (bool, i32), [usize;2]> = DynBox::empty([0usize;2]);
        let mut flag : bool = false;
        let mut cnt = 0;

        ptr.occupy(move || {
            flag = true;
            cnt += 1;
            (flag, cnt)
        });

        let mut ptr = outer(ptr);
        let (flag, cnt) = ptr.deref_mut()();


        assert_eq!(cnt, 3);
        assert_eq!(flag, true);
    }

    #[test] 
    fn test_gen() {
        struct Valve {
            pub reg : u32,
        }

        let mut v = Valve { reg : 0, };

        let mut sb : DynBox< dyn Generator<Yield = u32, Return = !> + core::marker::Unpin, [usize;0x10]> = DynBox::empty([0;0x10]);
        DynBox::occupy(&mut sb,
            || {
                loop {
                    yield 0u32;
                    v.reg = 1;
                    yield 1;
                    v.reg = 2;
                    yield 2;
                }
            }
        );

        for _ in 0 .. 6 {
            match Pin::new(&mut *sb).resume() {
                GeneratorState::Yielded(num) => { println!("Step : {}", num); }
                GeneratorState::Complete(_) => { println!("Finish step!"); }
            }
        }

        assert_eq!(v.reg, 2);
    }

}
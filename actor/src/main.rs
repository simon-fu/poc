// #![feature(thin_box)]
// #![feature(ptr_metadata)]
// use actix::{Actor, Context, System};
use actix::prelude::*;

pub mod actor; 

#[inline]
pub fn spawn_with_name<I, T>(name: I, fut: T) -> tokio::task::JoinHandle<T::Output>
where
    I: Into<String>,
    T: futures::Future + Send + 'static,
    T::Output: Send + 'static,
{
    let name:String = name.into();
    let span = tracing::span!(parent:None, tracing::Level::INFO, "", s = &name[..]);
    tokio::spawn(tracing::Instrument::instrument(fut, span))
}

struct MyActor;

impl Actor for MyActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("I am alive!");
        System::current().stop(); // <- stop system
    }
}



// this is our Message
// we have to define the response type (rtype)
#[derive(Message)]
#[rtype(result = "usize")]
struct Sum(usize, usize);

// Actor definition
struct Calculator;

impl Actor for Calculator {
    type Context = Context<Self>;
}

// now we need to implement `Handler` on `Calculator` for the `Sum` message.
impl Handler<Sum> for Calculator {
    type Result = usize; // <- Message response type

    fn handle(&mut self, msg: Sum, _ctx: &mut Context<Self>) -> Self::Result {
        msg.0 + msg.1
    }
}



fn main() {
    let system = System::new();

    println!("Hello, world!");
    let _addr = system.block_on(async { 
        MyActor.start() 
    });

    let _addr = system.block_on(async { 
        let addr = Calculator.start();
        let res = addr.send(Sum(10, 5)).await; // <- send message and get future for result
    
        match res {
            Ok(result) => println!("SUM: {}", result),
            _ => println!("Communication to the actor has failed"),
        }
    });



    system.run().unwrap();
}


#[cfg(test)]
mod test {
    use std::{mem::size_of_val, ops::Deref};
    
    

    pub trait Handler<M> {
        type Result ;
        fn handle(&mut self, msg: M) -> Self::Result;
    }

    // pub struct Envelope<A: Actor>(TBox<dyn EnvelopeProxy<A> + Send>);

    struct Five;

    impl Handler<i32> for Five {
        type Result = ();

        fn handle(&mut self, msg: i32) -> Self::Result {
            println!("i32 {}", msg);
        }
    }


    

    trait Say {
        fn say(&self, s: &str);
    }

    type TBox<T> = Box<T>;
    
    #[inline]
    fn box_trait<T: Say + 'static>(v: T) -> TBox<dyn Say> {
        Box::new(v)
    }

    #[inline]
    fn box_type<T: 'static>(v: T) -> TBox<T> {
        Box::new(v)
    }


    // type TBox<T> = std::boxed::ThinBox<T>;

    // #[inline]
    // fn box_trait<T: Say + 'static>(v: T) -> TBox<dyn Say> {
    //     std::boxed::ThinBox::new_unsize(v)
    // }

    // #[inline]
    // fn box_type<T: 'static>(v: T) -> TBox<T> {
    //     std::boxed::ThinBox::new(v)
    // }


    

    #[test]
    fn test() {
        
        struct Dummy1;
        impl Say for Dummy1 {
            fn say(&self, s: &str) {
                let ptr = self as *const Self;
                let func = Self::say as *const ();
                println!("Dummy1 [{:?}] say: [{}], func [{:?}]", ptr, s, func);
            }
        }
    
        struct Dummy2;
        impl Say for Dummy2 {
            fn say(&self, s: &str) {
                let ptr = self as *const Self;
                let func = Self::say as *const ();
                println!("Dummy2 [{:?}] say: [{}], func [{:?}]", ptr, s, func);
            }
        }

        struct Foo(String);
        impl Say for Foo {
            fn say(&self, s: &str) {
                let ptr = self as *const Self;
                let func = Self::say as *const ();
                println!("Foo [{:?}]-[{}] say: [{}], func {:?}", ptr, self.0, s, func);
            }
        }

        struct Bar(String);
        impl Say for Bar {
            fn say(&self, s: &str) {
                let ptr = self as *const Self;
                let func = Self::say as *const ();
                println!("Bar [{:?}]-[{}] say: [{}], func {:?}", ptr, self.0, s, func);
            }
        }

        impl Say for () {
            fn say(&self, s: &str) {
                let ptr = self as *const Self;
                let func = Self::say as *const ();
                println!("() [{:?}] say: [{}], func {:?}", ptr, s, func);
            }
        }

    
        let dummy0 = box_trait(Dummy1);
        println!("dummy0: ptr=[{:?}], size=[{}]", dummy0.deref() as *const dyn Say, size_of_val(&dummy0));

        let dummy1: TBox<dyn Say> = box_trait(Dummy1);
        println!("dummy1: ptr=[{:?}], size=[{}]", dummy1.deref() as *const dyn Say, size_of_val(&dummy1));
    
        let dummy2: TBox<dyn Say> = box_trait(Dummy2);
        println!("dummy2: ptr=[{:?}], size=[{}]", dummy2.deref() as *const dyn Say, size_of_val(&dummy2));

        let dummy3 = box_type(Dummy1);
        println!("dummy3: ptr=[{:?}], size=[{}]", dummy3.deref() as *const dyn Say, size_of_val(&dummy3));
    
        let dummy4 = box_type(Dummy2);
        println!("dummy4: ptr=[{:?}], size=[{}]", dummy4.deref() as *const dyn Say, size_of_val(&dummy4));

        let foo: TBox<dyn Say> = box_trait(Foo("foo".into()));
        println!("foo=[{:?}]", foo.deref() as *const dyn Say);

        let bar: TBox<dyn Say> = box_trait(Bar("bar".into()));
        println!("bar=[{:?}]", bar.deref() as *const dyn Say);


    
        say_it(dummy1, "hello");
        say_it(dummy2, "hello");
        say_it(foo, "hello");
        say_it(bar, "hello");

    }

    fn say_it(handler: TBox<dyn Say>, s: &str) {
        println!("handler=[{:?}]", (handler.deref() as *const dyn Say).to_raw_parts());
        handler.say(s);
    }
    
}

use std::io;
use std::io::Read;

fn main() {
    println!("Hello and burn this world!");

    #[cfg(feature = "some_feature")]
    {
        println!("I'm using some_feature!")
    }

    #[cfg(not(feature = "some_feature"))]
    {
        println!("I'm not using some_feature!")
    }

    println!("Press anything to exit...");
    io::stdin().read_exact(&mut [0u8]).unwrap();
}

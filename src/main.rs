use board_id::BoardId;

fn main() {
    println!("{}", BoardId::detect().unwrap());
}

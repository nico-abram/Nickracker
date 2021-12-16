use text_io::read;

use solver::SolverContext;

const SYMBOL_IDX_TO_LETTER: [&'static str; solver::SYMBOL_COUNT] = [
    "Inverted J? The first symbol",
    "B",
    "C",
    "F",
    "> and < on top of each other (NOT the X, they cross!)",
    "M",
    "P",
    "The thing between P and S under B",
    "S (Jagged, like a thunder)",
    "T (Like a cross, the upper line has an angle downwards)",
    "|X| (Between the T and X)",
    "X",
];

fn main() {
    let mut state = SolverContext::new();

    loop {
        state.reset();
        loop {
            let guess = state.guess();
            println!(
                "Please try this guess: \n\t{:?}",
                guess
                    .iter()
                    .map(|&symbol_idx| SYMBOL_IDX_TO_LETTER[symbol_idx as usize])
                    .collect::<Vec<_>>()
            );
            println!("What was the result? (Enter the 2 numbers, press enter after each)");
            let one: u8 = read!();
            if one == 4 {
                println!("Found it!");
                println!("Press Q to quit, anything else to guess another puzzle");
                let mut line = String::new();
                let stdin = std::io::stdin();
                std::io::BufRead::read_line(&mut stdin.lock(), &mut line).unwrap();
                drop(stdin);
                if line == "Q" {
                    return;
                }
                break;
            }
            let two: u8 = read!();
            println!("({}, {})", one, two);
            state.apply_result(guess, one, two);
        }
    }
}

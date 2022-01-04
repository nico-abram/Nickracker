// Ignore this macro, it is black magic
macro_rules! vec_to_arr_box {
    (
    $vec_expr:expr, $size_expr:expr
) => {{
        const N: usize = $size_expr;
        let vec_expr = $vec_expr;
        {
            use std::prelude::v1::*;

            #[inline]
            fn with_ty<T>(v: Vec<T>) -> Box<[T; N]> {
                let boxed_slice: Box<[T]> = Vec::into_boxed_slice(v);
                assert_eq!(<[T]>::len(&*boxed_slice), N);
                let raw: *mut [T] = Box::into_raw(boxed_slice);
                unsafe { Box::from_raw(raw as *mut [T; N]) }
            }

            with_ty(vec_expr)
        }
    }};
}

pub const SYMBOL_COUNT: usize = 12;
pub const ANSWER_SIZE: usize = 4;
pub const POSSIBLE_ANSWERS: usize = SYMBOL_COUNT.pow(ANSWER_SIZE as u32);

pub type PossibleAnswer = [u8; ANSWER_SIZE];

// We apply  Donald Knuth's algorithm
// Implemented in SolverContext::apply_result(ans: PossibleAnswer, correct_positions: u8,
// correct_symbols: u8) and SolverContext::guess() -> PossibleAnswer
//
// PossibleAnswer is an array of 4 numbers, for the 4 symbols. I also use a second, more compact
// representation of it with a single number/index/idx. These 2 functions convert between them
pub fn idx_to_answer(idx: usize) -> PossibleAnswer {
    [
        ((idx / SYMBOL_COUNT.pow(0)) % SYMBOL_COUNT) as u8,
        ((idx / SYMBOL_COUNT.pow(1)) % SYMBOL_COUNT) as u8,
        ((idx / SYMBOL_COUNT.pow(2)) % SYMBOL_COUNT) as u8,
        ((idx / SYMBOL_COUNT.pow(3)) % SYMBOL_COUNT) as u8,
    ]
}
pub fn answer_to_idx(ans: PossibleAnswer) -> usize {
    for ans in ans.iter() {
        debug_assert!((*ans as usize) < SYMBOL_COUNT);
    }
    ans[0] as usize * SYMBOL_COUNT.pow(0)
        + ans[1] as usize * SYMBOL_COUNT.pow(1)
        + ans[2] as usize * SYMBOL_COUNT.pow(2)
        + ans[3] as usize * SYMBOL_COUNT.pow(3)
}

/// Replaced by a faster compare below
pub fn compare2(a_and_b: &[PossibleAnswer; 2]) -> (u8, u8) {
    let a = &a_and_b[0];
    let b = &a_and_b[1];

    let mut found_match_at_pos = [false; ANSWER_SIZE];
    let mut correct_positions = 0;
    for ((idx, aa), bb) in a.iter().enumerate().zip(b.iter()) {
        // Same symbol, same position
        if aa == bb {
            correct_positions += 1;
            found_match_at_pos[idx] = true;
        }
    }
    let mut a_symbol_counter = [0u8; SYMBOL_COUNT];
    let mut b_symbol_counter = [0u8; SYMBOL_COUNT];
    for (aa, already_matched) in a.iter().zip(found_match_at_pos.iter()) {
        if !already_matched {
            a_symbol_counter[*aa as usize] += 1;
        }
        // Same symbol, any position
        for (bb, already_matched) in b.iter().zip(found_match_at_pos.iter()) {
            if !already_matched && aa == bb {
                b_symbol_counter[*bb as usize] += 1;
                break;
            }
        }
    }
    let correct_symbols = a_symbol_counter
        .into_iter()
        .zip(b_symbol_counter.into_iter())
        .map(|(a, b)| a.min(b))
        .sum::<u8>();

    debug_assert!(correct_positions <= SYMBOL_COUNT as u8);
    debug_assert!(correct_symbols <= SYMBOL_COUNT as u8);

    (correct_positions, correct_symbols)
}

/// Compare 2 answers/guesses Returns a pair of (correct_positions, correct_symbols)
pub fn compare(a_and_b: &[PossibleAnswer; 2]) -> (u8, u8) {
    let a = &a_and_b[0];
    let b = &a_and_b[1];

    // https://mathworld.wolfram.com/Mastermind.html.
    // whites = sum(min(a_i, b_i)) - blacks
    // Where whites=correct_symbols and blacks=correct_positions
    // and a_i/b_i mean "Count of symbol i in a/b"
    let mut correct_positions = 0;
    let mut a_symbol_counter = [0u8; SYMBOL_COUNT];
    let mut b_symbol_counter = [0u8; SYMBOL_COUNT];
    for (aa, bb) in a.iter().zip(b.iter()) {
        // Same symbol, same position
        if aa == bb {
            correct_positions += 1;
        }
        // There's bounds checking here. I can get rid of it in safe code making the arrays 16 elems
        // and masking with 15 but it does not currently change perf
        b_symbol_counter[*bb as usize] += 1;
        a_symbol_counter[*aa as usize] += 1;
    }
    let correct_symbols = a_symbol_counter
        .iter()
        .zip(b_symbol_counter.iter())
        .map(|(a, b)| a.min(b))
        .sum::<u8>()
        - correct_positions;

    (correct_positions, correct_symbols)
}
pub struct SolverContext {
    answers_known_to_be_false: Box<[bool; POSSIBLE_ANSWERS]>,
    attempt: u8,
    first_idx: usize,
    last_idx: usize,
}

impl Default for SolverContext {
    fn default() -> Self {
        Self::new()
    }
}

impl SolverContext {
    pub fn new() -> Self {
        Self {
            answers_known_to_be_false: vec_to_arr_box!(
                vec![false; POSSIBLE_ANSWERS],
                POSSIBLE_ANSWERS
            ),
            first_idx: 0,
            last_idx: POSSIBLE_ANSWERS,
            attempt: 0,
        }
    }
    pub fn apply_result(
        &mut self,
        ans: PossibleAnswer,
        correct_positions: u8,
        correct_symbols: u8,
    ) {
        let mut compare_area = [ans, ans];

        for (idx, mut_ref_to_known_false) in self.answers_known_to_be_false
            [self.first_idx..self.last_idx]
            .iter_mut()
            .enumerate()
            .filter(|&(_, &mut known_bad)| !known_bad)
        {
            let idx = idx + self.first_idx;
            compare_area[0] = idx_to_answer(idx);
            if compare(&compare_area) != (correct_positions, correct_symbols) {
                *mut_ref_to_known_false = true;
            }
        }
        self.first_idx = self
            .answers_known_to_be_false
            .iter()
            .enumerate()
            .filter(|&(_, &known_bad)| !known_bad)
            .map(|(idx, _)| idx)
            .next()
            .unwrap();
        self.last_idx = self
            .answers_known_to_be_false
            .iter()
            .enumerate()
            .rev()
            .filter(|&(_, &known_bad)| !known_bad)
            .map(|(idx, _)| idx)
            .next()
            .unwrap()
            + 1;
    }
    pub fn guess(&mut self) -> PossibleAnswer {
        self.attempt += 1;
        // For the first attempt, I think any pattern of the form [A, A, B, B] is equally as good
        // Special case it because since it has the biggest search space (Full 12**4) it is the
        // slowest
        if self.attempt == 1 {
            return [0, 0, 1, 2];
        }
        if self.attempt == 2 {
            return [3, 3, 4, 4];
        }
        if self.attempt == 3 {
            //return [4,4,5,5];
            return [5, 5, 6, 6];
        }
        // We find the guess that, in the worst case, leaves us with the smallest possible
        // remaining set of possible solutions (We find it by brute force)
        let mut guess = 1;
        let mut possibilities_left_after = POSSIBLE_ANSWERS;
        for (idx, _) in self.answers_known_to_be_false[self.first_idx..self.last_idx]
            .iter()
            .enumerate()
            .filter(|&(_, &known_bad)| !known_bad)
        {
            let idx = idx + self.first_idx;
            let mut max_possible_answers = 0usize;
            for correct_positions in 0..3 {
                for correct_symbols in 0..4 {
                    if correct_positions + correct_symbols < 5 {
                        // Skip invalid
                        max_possible_answers =
                            max_possible_answers.max(self.calc_guess_leftover_possibilities(
                                idx,
                                correct_positions,
                                correct_symbols,
                            ));
                    }
                }
            }
            if max_possible_answers < possibilities_left_after {
                guess = idx;
                possibilities_left_after = max_possible_answers;
            }
        }
        idx_to_answer(guess)
    }
    fn calc_guess_leftover_possibilities(
        &self,
        idx: usize,
        correct_positions: u8,
        correct_symbols: u8,
    ) -> usize {
        let mut compare_area = [idx_to_answer(idx), idx_to_answer(idx)];
        let mut possible_answer_count = 0;

        for (possible_idx, _) in self.answers_known_to_be_false[self.first_idx..self.last_idx]
            .iter()
            .enumerate()
            .filter(|&(_, &known_bad)| !known_bad)
        {
            let possible_idx = possible_idx + self.first_idx;
            compare_area[0] = idx_to_answer(possible_idx);
            if compare(&compare_area) != (correct_positions, correct_symbols) {
                possible_answer_count += 1;
            }
        }
        possible_answer_count
    }
    pub fn reset(&mut self) {
        self.answers_known_to_be_false[..].fill(false);
        self.attempt = 0;
        self.first_idx = 0;
        self.last_idx = POSSIBLE_ANSWERS;
    }
    pub fn solve(&mut self, actual_secret_answer: PossibleAnswer) -> usize {
        let mut compare_area = [actual_secret_answer; 2];
        self.reset();

        let mut try_count = 0;
        let mut last_guess = None;
        loop {
            try_count += 1;
            let guess = self.guess();
            if Some(guess) == last_guess {
                panic!(
                    "Tried the same guess ({:?}) twice wtf! secret was {:?}",
                    guess, actual_secret_answer
                );
            }

            if guess == actual_secret_answer {
                return try_count;
            }

            compare_area[1] = guess;
            let res = compare(&compare_area);
            self.apply_result(guess, res.0, res.1);
            last_guess = Some(guess);
        }
    }
}

pub fn basic_test() {
    let mut state = SolverContext::new();
    let answer = [3, 4, 5, 1];
    dbg!(state.solve(answer));
}
pub fn noisy_solve(secret: PossibleAnswer) {
    let mut state = SolverContext::new();
    let mut compare_area = [secret; 2];

    let mut try_count = 0;
    let mut last_guess = None;
    loop {
        try_count += 1;
        let guess = state.guess();
        println!(
            "Guess {}: {:?} (Remaining {} (raw {} F{} L{} len {}) posibilities) {:?}: {}",
            try_count,
            guess,
            state.answers_known_to_be_false[state.first_idx..state.last_idx]
                .iter()
                .filter(|&known_bad| !known_bad)
                .count(),
            state
                .answers_known_to_be_false
                .iter()
                .filter(|&known_bad| !known_bad)
                .count(),
            state.first_idx,
            state.last_idx,
            state.answers_known_to_be_false[state.first_idx..state.last_idx].len(),
            secret,
            state.answers_known_to_be_false[answer_to_idx(secret)]
        );
        if Some(guess) == last_guess {
            panic!("Tried the same guess twice wtf!");
        }

        if guess == secret {
            println!("Found");
            return;
        }

        compare_area[1] = guess;
        let res = compare(&compare_area);
        state.apply_result(guess, res.0, res.1);
        println!("compare: {:?}", res);
        last_guess = Some(guess);
    }
}
#[cfg(test)]
mod test {
    use super::*;

    // These tests are actually kind of the most important ones, make sure our comparator matches
    // the game
    #[test]
    fn compares() {
        assert_eq!(compare(&[[0, 5, 0, 3], [3, 3, 4, 4]]), (0, 1));
        assert_eq!(compare(&[[0, 5, 0, 3], [0, 0, 1, 2]]), (1, 1));
        assert_eq!(compare(&[[5, 4, 2, 0], [0, 0, 1, 1]]), (0, 1));
        assert_eq!(compare(&[[5, 0, 2, 3], [0, 0, 1, 1]]), (1, 0));
        assert_eq!(compare(&[[0, 3, 4, 4], [0, 0, 1, 1]]), (1, 0));
        assert_eq!(compare(&[[3, 1, 0, 4], [0, 0, 1, 1]]), (0, 2));
    }
    #[test]
    fn solves() {
        let mut solver = SolverContext::new();
        solver.solve([0, 5, 0, 3]);
        solver.solve([3, 1, 0, 4]);
        solver.solve([5, 0, 2, 3]);
        solver.solve([0, 3, 4, 4]);
        solver.solve([5, 4, 2, 0]);
        /*
            noisy_solve([0, 5, 0, 3]);
            noisy_solve([3, 1, 0, 4]);
            noisy_solve([5, 0, 2, 3]);
            noisy_solve([0, 3, 4, 4]);
            noisy_solve([5, 4, 2, 0]);
        */
    }

    // Testing everything takes too long, only do it every so often. This should do for the most
    // part
    #[test]
    #[ignore = "too slow"]
    fn exhaustive() {
        exhaustive_test(0, POSSIBLE_ANSWERS);
    }
    const ONE_FIFTH: usize = POSSIBLE_ANSWERS / 5;
    #[test]
    #[ignore = "too slow"]
    fn exhaustive_one_fifths() {
        exhaustive_test(0, ONE_FIFTH);
    }
    #[test]
    #[ignore = "too slow"]
    fn exhaustive_two_fifths() {
        exhaustive_test(ONE_FIFTH, ONE_FIFTH * 2);
    }
    #[test]
    #[ignore = "too slow"]
    fn exhaustive_three_fifths() {
        exhaustive_test(ONE_FIFTH * 2, ONE_FIFTH * 3);
    }
    #[test]
    #[ignore = "too slow"]
    fn exhaustive_four_fifths() {
        exhaustive_test(ONE_FIFTH * 3, ONE_FIFTH * 4);
    }
    #[test]
    #[ignore = "too slow"]
    fn exhaustive_five_fifths() {
        exhaustive_test(ONE_FIFTH * 4, POSSIBLE_ANSWERS);
    }

    #[test]
    fn not_quite_exhaustive() {
        const N: usize = POSSIBLE_ANSWERS / 128;
        // First N
        exhaustive_test(0, N);
        // Last N
        exhaustive_test(POSSIBLE_ANSWERS - N, POSSIBLE_ANSWERS);
    }
    // Test all possible answers
    fn exhaustive_test(first: usize, last: usize) {
        use std::cell::RefCell;
        use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

        let progress = AtomicUsize::new(0);
        let max_tries = AtomicUsize::new(0);
        let max_secret = AtomicU32::new(0);
        let state: thread_local::ThreadLocal<RefCell<SolverContext>> =
            thread_local::ThreadLocal::new();

        use rayon::prelude::*;

        const ZERO: AtomicUsize = AtomicUsize::new(0);
        let guess_count_bins = [ZERO; 16];
        let print_progress = |progress| {
            println!(
                "Progress {}/{} current max tries:{} current worst guess:{:?}",
                progress,
                last - first,
                max_tries.load(Ordering::SeqCst),
                u32::to_ne_bytes(max_secret.load(Ordering::SeqCst))
            );
        };
        let m = std::sync::Mutex::new(());
        (first..last)
            .into_iter()
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|i| {
                let mut state = state
                    .get_or(|| RefCell::new(SolverContext::new()))
                    .try_borrow_mut()
                    .unwrap();
                let p = progress.fetch_add(1, Ordering::Relaxed);
                if p % 32 == 0 {
                    print_progress(p);
                }

                let secret = idx_to_answer(i);
                let try_count = state.solve(secret);

                if max_tries.load(Ordering::Relaxed) < try_count {
                    let _l = m.lock();
                    if max_tries.load(Ordering::SeqCst) < try_count {
                        max_tries.store(try_count, Ordering::SeqCst);
                        max_secret.store(u32::from_ne_bytes(secret), Ordering::SeqCst);
                    }
                }
                guess_count_bins[try_count].fetch_add(1, Ordering::Relaxed);
            });
        print_progress(progress.load(Ordering::SeqCst));
        println!(
            "guess_count_bins: {:?}",
            guess_count_bins
                .iter()
                .enumerate()
                .skip(1)
                .map(|(idx, x)| format!("{} tries:{}", idx, x.load(Ordering::SeqCst)))
                .collect::<Vec<_>>()
        );
        let avg: f64 = guess_count_bins
            .iter()
            .enumerate()
            .skip(1)
            .map(|(idx, x)| idx as f64 * x.load(Ordering::SeqCst) as f64 / ((last - first) as f64))
            .sum();
        println!("avg: {}", avg);
    }
}

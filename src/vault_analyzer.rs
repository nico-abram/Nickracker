/// This file takes care of "analyzing" a screenshot of the game and getting the current vault
/// puzzle state from it. The output is the [AnalyzedMinotaurVault] type
use crate::{bmp, ocr, ANSWER_SIZE, MAX_GUESSES};

use std::sync::atomic::{AtomicUsize, Ordering};

/// The results of analyzing a screenshot with a minotaur vault window
#[derive(Debug, PartialEq, Eq)]
pub struct AnalyzedMinotaurVault {
    /// The symbols currently being selected for the next guess
    selected_symbols: [Option<u8>; ANSWER_SIZE],
    /// The guesses made in the past: The 4 symbols for each and the 2 resulting numbers
    made_guesses: [Option<([u8; ANSWER_SIZE], u8, u8)>; MAX_GUESSES - 1],
}
impl AnalyzedMinotaurVault {
    fn selected_iter(&self) -> impl Iterator<Item = usize> + '_ {
        let mut it = self.selected_symbols.iter().cloned();
        std::iter::from_fn(move || it.next().flatten().map(|x| x as usize))
    }
    fn guesses_iter(&self) -> impl Iterator<Item = ([u8; ANSWER_SIZE], usize, usize)> + '_ {
        let mut it = self.made_guesses.iter().cloned();
        std::iter::from_fn(move || {
            it.next()
                .flatten()
                .map(|x| (x.0, x.1 as usize, x.2 as usize))
        })
    }
}

pub struct VaultAnalyzerCtx {
    // "X" window close button image used to find windows
    window_x: Vec<u8>,
    window_x_grayscale: Vec<u8>,
    window_x_w: usize,
    window_x_h: usize,

    // Comma image used to find past guess result positions
    comma: Vec<u8>,
    comma_w: usize,
    comma_h: usize,
    comma_grayscale: Vec<u8>,

    /// Images of small versions of the symbols. Can have more than 1 per sym
    /// ((bytes, w, h), symbol_idx)
    smol_symbols: Vec<(Vec<u8>, usize, usize)>,
    smol_symbols_gray: Vec<(Vec<u8>, usize, usize)>,

    big_symbols: Vec<(Vec<u8>, usize, usize)>,
    big_symbols_gray: Vec<(Vec<u8>, usize, usize)>,

    finder: find_subimage::SubImageFinderState,

    ocr: ocr::OcrState,
}
impl VaultAnalyzerCtx {
    pub fn new() -> Option<Self> {
        let smol_symbols: [(&[u8], usize); 12] = [
            (include_bytes!("bmps/symbol0_smol.bmp"), 0),
            (include_bytes!("bmps/symbol1_smol.bmp"), 1),
            (include_bytes!("bmps/symbol2_smol.bmp"), 2),
            (include_bytes!("bmps/symbol3_smol.bmp"), 3),
            (include_bytes!("bmps/symbol4_smol.bmp"), 4),
            (include_bytes!("bmps/symbol5_smol.bmp"), 5),
            (include_bytes!("bmps/symbol6_smol.bmp"), 6),
            (include_bytes!("bmps/symbol7_smol.bmp"), 7),
            (include_bytes!("bmps/symbol8_smol.bmp"), 8),
            (include_bytes!("bmps/symbol9_smol.bmp"), 9),
            (include_bytes!("bmps/symbol10_smol.bmp"), 10),
            (include_bytes!("bmps/symbol11_smol.bmp"), 11),
        ];
        let smol_symbols = smol_symbols
            .iter()
            .map(|(bytes, _n)| bmp::parse_rgb_bmp(bytes).unwrap())
            .collect::<Vec<_>>();
        let smol_symbols_gray = smol_symbols
            .iter()
            .map(|(b, w, h)| (to_grayscale(b), *w, *h))
            .collect::<Vec<_>>();

        let big_symbols: [(&[u8], usize); 12] = [
            (include_bytes!("bmps/symbol0.bmp"), 0),
            (include_bytes!("bmps/symbol1.bmp"), 1),
            (include_bytes!("bmps/symbol2.bmp"), 2),
            (include_bytes!("bmps/symbol3.bmp"), 3),
            (include_bytes!("bmps/symbol4.bmp"), 4),
            (include_bytes!("bmps/symbol5.bmp"), 5),
            (include_bytes!("bmps/symbol6.bmp"), 6),
            (include_bytes!("bmps/symbol7.bmp"), 7),
            (include_bytes!("bmps/symbol8.bmp"), 8),
            (include_bytes!("bmps/symbol9.bmp"), 9),
            (include_bytes!("bmps/symbol10.bmp"), 10),
            (include_bytes!("bmps/symbol11.bmp"), 11),
        ];
        let big_symbols = big_symbols
            .iter()
            .map(|(bytes, _n)| bmp::parse_rgb_bmp(bytes).unwrap())
            .collect::<Vec<_>>();
        let big_symbols_gray = big_symbols
            .iter()
            .map(|(b, w, h)| (to_grayscale(b), *w, *h))
            .collect::<Vec<_>>();

        let (comma, comma_w, comma_h) =
            bmp::parse_rgb_bmp(include_bytes!("bmps/comma_gray.bmp")).ok()?;
        let comma_grayscale = to_grayscale(&comma);

        let (window_x, window_x_w, window_x_h) =
            bmp::parse_rgb_bmp(include_bytes!("bmps/x.bmp")).ok()?;
        let window_x_grayscale = to_grayscale(&window_x);

        Some(Self {
            window_x,
            window_x_w,
            window_x_h,
            window_x_grayscale,

            comma,
            comma_w,
            comma_h,
            comma_grayscale,

            ocr: ocr::OcrState::new()?,
            finder: find_subimage::SubImageFinderState::new(),

            smol_symbols,
            smol_symbols_gray,

            big_symbols,
            big_symbols_gray,
        })
    }

    pub fn find_minotaur_vault(
        &mut self,
        rgb_bytes: &[u8],
        width: usize,
        height: usize,
    ) -> Option<(AnalyzedMinotaurVault, usize, usize)> {
        find_minotaur_vault_impl(self, (rgb_bytes, width, height))
    }

    fn configure_finder(
        &mut self,
        (step_x, step_y, threshold): (usize, usize, f32),
        (p_w, p_h): (f32, f32),
    ) {
        self.finder
            .set_backend(find_subimage::Backend::RuntimeDetectedSimd {
                step_x,
                step_y,
                threshold,
            });
        self.finder.set_pruning(p_w, p_h);
    }
}

//// Returns (title, left, right, top)
fn get_possible_window_borders_and_title(
    ocr: &mut ocr::OcrState,
    pos: (usize, usize),
    screenshot: &[u8],
    screenshot_w: usize,
) -> Option<(String, usize, usize, usize)> {
    const THRESHOLD: usize = 0xB;

    // usize color into it's 3 rgb components
    let into_parts = |x: usize| ((x >> 16) & 0xFF, (x >> 8) & 0xFF, x & 0xFF);

    // Check each component against threshold
    let matches = |(r, g, b)| r < THRESHOLD && g < THRESHOLD && b < THRESHOLD;

    // Check that each color has a difference below the threshold
    let compare = |left: usize, right: usize| {
        let (r, g, b) = into_parts(left);
        let (r2, g2, b2) = into_parts(right);
        matches((
            (r as isize - r2 as isize).abs() as usize,
            (g as isize - g2 as isize).abs() as usize,
            (b as isize - b2 as isize).abs() as usize,
        ))
    };

    // Sum of individual color distances between 2 rgb colors
    let dist = |left: usize, right: usize| {
        let (r, g, b) = into_parts(left);
        let (r2, g2, b2) = into_parts(right);
        (r as isize - r2 as isize).abs() as usize
            + (g as isize - g2 as isize).abs() as usize
            + (b as isize - b2 as isize).abs() as usize
    };

    // Find the top right corner by moving upwards and finding the closest pixel to the window
    // border
    // TODO: The actual top right needs to be pos.0 + window_x_width
    let mut kinda_top_right_corner = pos;
    let mut min_dist = 999;
    for i in 0..30 {
        if pos.1 < i {
            break;
        }
        let new_pos = (pos.0, pos.1 - i);
        let pos_search = (new_pos.0 * 3 + new_pos.1 * screenshot_w * 3) as usize;
        let color = ((screenshot[pos_search + 0] as usize) << 16)
            | ((screenshot[pos_search + 1] as usize) << 8)
            | (screenshot[pos_search + 2] as usize);

        let dist = dist(color, 0x191011);
        if min_dist > dist {
            kinda_top_right_corner = new_pos;
            min_dist = dist;
        }
    }

    let border_colors = [
        0x191011, 0x1A1112, 0x240F14, 0x19110F, 0x1B1311, 0x4A3A3A, 0x594747, 0x262221, 0x312726,
        0x252115, 0x41322D, 0x3F3131, 0x433032, 0x403136, 0x3B342C, 0x472C31, 0x393333, 0x403635,
        0x382D2B, 0x413232, 0x231D1D, 0x2F2523, 0x2F2725, 0x312525, 0x17141D,
    ];
    let is_border = |color| border_colors.into_iter().any(|x| compare(x, color));

    // Move to the left until we are no longer in the window border to find the top left window edge
    let mut x_off = 0;
    let kinda_top_left_corner = loop {
        let pos = kinda_top_right_corner;
        if pos.0 < x_off {
            return None;
        }
        let pos_search = ((pos.0 - x_off) * 3 + pos.1 * screenshot_w * 3) as usize;
        let color = ((screenshot[pos_search + 0] as usize) << 16)
            | ((screenshot[pos_search + 1] as usize) << 8)
            | (screenshot[pos_search + 2] as usize);

        if !is_border(color) {
            break (pos.0 - x_off, pos.1);
        }
        x_off += 1;
    };

    const BORDER_OFFSET: usize = 3;
    let kinda_bottom_right_title_ocr = (
        // Remove some whitespace
        kinda_top_right_corner.0 - 16,
        // Add line and matching offset (See below)
        kinda_top_right_corner.1 + 31 + BORDER_OFFSET,
    );

    let kinda_top_left_corner = (
        // Remove some whitespace
        kinda_top_left_corner.0 + 8,
        // Remove a bit of the border
        kinda_top_left_corner.1 + BORDER_OFFSET,
    );

    if kinda_bottom_right_title_ocr.0 <= kinda_top_left_corner.0 {
        return None;
    }

    // Extract image fragment for OCR
    // TODO: might not be needed, can maybe just use the correct stride?
    // Would need to pre-invert the entire screenshot, not sure if worth it
    // In case I do, already got the X image inverted
    let w = kinda_bottom_right_title_ocr.0 - kinda_top_left_corner.0;
    let h = kinda_bottom_right_title_ocr.1 - kinda_top_left_corner.1;
    let mut fragment = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let new_idx = x * 3 + y * w * 3;
            let orig_idx = (kinda_top_left_corner.0 + x) * 3
                + (kinda_top_left_corner.1 + y) * screenshot_w * 3;

            // Invert image colors, seems to help tesseract
            // See here: https://tesseract-ocr.github.io/tessdoc/ImproveQuality.html#inverting-images
            fragment[new_idx + 0] = 255 - screenshot[orig_idx + 0];
            fragment[new_idx + 1] = 255 - screenshot[orig_idx + 1];
            fragment[new_idx + 2] = 255 - screenshot[orig_idx + 2];
        }
    }

    let title = ocr
        .ocr_generic(&fragment[..], w, h, None, kinda_top_left_corner)?
        .trim()
        .to_string();

    Some((
        title,
        kinda_top_left_corner.0,
        kinda_bottom_right_title_ocr.0,
        kinda_top_left_corner.1,
    ))
}

fn to_grayscale(b: &[u8]) -> Vec<u8> {
    b.chunks_exact(3)
        .map(|rgb| rgb.iter().map(|x| (*x as f32) / 3.0).sum::<f32>() as u8)
        .collect::<Vec<u8>>()
}

fn find_minotaur_vault_impl(
    ctx: &mut VaultAnalyzerCtx,
    (ss, ss_w, ss_h): (&[u8], usize, usize),
) -> Option<(AnalyzedMinotaurVault, usize, usize)> {
    // Used for debug output screenshot filenames
    static ANALYZED_VAULT_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let analyzed_vault_counter = ANALYZED_VAULT_COUNTER.fetch_add(1, Ordering::Relaxed);

    let ss_grayscale = to_grayscale(ss);

    //ctx.configure_finder((2, 2, 0.01), (0.5, 0.5));
    ctx.configure_finder((1, 1, 0.02), (0.5, 0.5));
    let window_x_locs = ctx.finder.find_subimage_positions(
        (&ss_grayscale, ss_w, ss_h),
        (&ctx.window_x_grayscale, ctx.window_x_w, ctx.window_x_h),
        1,
    );
    if ocr::ENABLE_DEBUG_IMAGE_OUTPUT {
        bmp::save_rgb_bmp(
            ss,
            ss_w,
            ss_h,
            &format!("dbg/vault_ss_{}.bmp", analyzed_vault_counter),
        );
        bmp::save_gray_bmp(
            &ss_grayscale,
            ss_w,
            ss_h,
            &format!("dbg/vault_ss_gray_{}.bmp", analyzed_vault_counter),
        );
    }
    if ocr::DEBUG_CONSOLE_OUTPUT {
        println!("window_x_locs: {:?}", &window_x_locs);
    }

    let ocr_results = window_x_locs
        .iter()
        .filter_map(|&loc| {
            // For each match of the X subimage we found, attempt to find the window and OCR the
            // title
            get_possible_window_borders_and_title(&mut ctx.ocr, (loc.0, loc.1), ss, ss_w)
                // Then filter the windows that had the Minotaur Lock title
                .and_then(|x| {
                    if ocr::DEBUG_CONSOLE_OUTPUT {
                        println!(
                            "Found window \"{}\" at ({}, {}) right_x:{}",
                            &x.0, x.1, x.2, x.3
                        );
                    }
                    if x.0 == "Minotaur Lock" {
                        Some(x)
                    } else {
                        None
                    }
                })
        })
        .collect::<Vec<_>>();

    let (_title, left, right, top) = ocr_results.get(0)?;
    let (window_x, window_y) = (left + 30, top + 30);
    let window_w = right - left;

    // Now we try to find the bottom edge of the area with the results of past guesses that we care
    // about. To do this, we cast a line downwards until we find the white area, and then keep going
    // until we leave the white area. Because we sometimes end up in different X positions and not
    // always at the rightmost edge of the window, and because depending on the window size there
    // can be more or less padding between the first result box and the edge of the window, we cast
    // multiple lines and use the best result.

    // Somewhere between 250 and 290
    const SELECTOR_AREA_HEIGHT: usize = 250;
    // Height of just the current selection. The rest are the 12 symbol clickable buttons
    const SELECTOR_AREA_HEIGHT_ONLY_SELECTION: usize = 110;
    // We add some extra padding when searching for the white panel
    const SELECTOR_AREA_HEIGHT_PADDING: usize = 70;
    const SELECTOR_AREA_HEIGHT_W_PAD: usize = SELECTOR_AREA_HEIGHT + SELECTOR_AREA_HEIGHT_PADDING;
    let off_y = {
        // Color of the white subrectangle in the guess results area
        const WHITE_COL: isize = 0x90;
        const WHITE_COL_THRESHOLD: isize = 0x05;

        let cast_line_downwards_with_x_offset = |x_offset| {
            let mut off_y = SELECTOR_AREA_HEIGHT_W_PAD;
            let mut found_white = false;

            let starting_idx = ((window_x as isize + x_offset) as usize) + window_y * ss_w;
            loop {
                let col = ss_grayscale.get(starting_idx + off_y * ss_w);
                if let Some(&col) = col {
                    if !found_white {
                        if (col as isize - WHITE_COL).abs() < WHITE_COL_THRESHOLD {
                            found_white = true;
                            continue;
                        }
                        // If we go 30 pixels without finding white, bail out
                        if off_y > SELECTOR_AREA_HEIGHT_W_PAD + 30 {
                            off_y = SELECTOR_AREA_HEIGHT_W_PAD;
                            break;
                        }
                    } else if (col as isize - WHITE_COL).abs() >= WHITE_COL_THRESHOLD {
                        break;
                    }

                    off_y += 1;
                } else {
                    return 0;
                }
            }
            off_y + 15
        };

        cast_line_downwards_with_x_offset(20)
            .max(cast_line_downwards_with_x_offset(5))
            .max(cast_line_downwards_with_x_offset(0))
            .max(cast_line_downwards_with_x_offset(-10))
    };

    // We now split the window into two parts: The upper part with the "selector area" with the
    // currently selected symbols, and the bottom part with the "guess results area" with the past
    // guess results, and we extract a fragment of the screenshot each
    let guess_results_area_h = off_y - SELECTOR_AREA_HEIGHT;

    let mut guess_results_area_fragment = vec![0u8; window_w * guess_results_area_h];
    let starting_idx_in = window_x + (window_y + SELECTOR_AREA_HEIGHT) * ss_w;
    for y in 0..guess_results_area_h {
        for x in 0..window_w {
            let idx_in = starting_idx_in + x + y * ss_w;
            let idx_out = x + y * window_w;

            guess_results_area_fragment[idx_out] = ss_grayscale[idx_in];
        }
    }

    // And look for the comma subimage in it to find actual results of past guesses
    ctx.configure_finder((1, 1, 0.085), (3.0, 3.0));

    let mut past_guess_result_locations = ctx
        .finder
        .find_subimage_positions(
            (&guess_results_area_fragment, window_w, guess_results_area_h),
            (&ctx.comma_grayscale, ctx.comma_w, ctx.comma_h),
            1,
        )
        .to_owned();
    // Sort results by y and then x position
    past_guess_result_locations.sort_unstable_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    if ocr::DEBUG_CONSOLE_OUTPUT {
        println!("comma locs: {:?}", &past_guess_result_locations);
    }

    if ocr::ENABLE_DEBUG_IMAGE_OUTPUT {
        let mut outty = guess_results_area_fragment
            .iter()
            .cloned()
            .flat_map(|x| [x, x, x])
            .collect::<Vec<u8>>();
        for (x, y, _dist) in &past_guess_result_locations {
            let idx = x * 3 + y * window_w * 3;
            let mut mark = |idx| {
                // Red mark where we found matches
                outty[idx + 0] = 0;
                outty[idx + 1] = 0;
                outty[idx + 2] = 255;
            };
            mark(idx);
            mark(idx + 3);
            mark(idx + window_w * 3);
            mark(idx + 3 + window_w * 3);
        }
        bmp::save_rgb_bmp(
            &outty,
            window_w,
            guess_results_area_h,
            &format!(
                "dbg/test_vault_window_marked_{}.bmp",
                analyzed_vault_counter
            ),
        );
    }

    let mut result = AnalyzedMinotaurVault {
        selected_symbols: [None; ANSWER_SIZE],
        made_guesses: [None; MAX_GUESSES - 1],
    };

    let parse_guess = |s: &str| -> Option<(u8, u8)> {
        let mut chars = s.chars();
        let a = chars.next()?.to_digit(10)?;
        // Sometimes the second character is a comma instead of a number
        let mut b = chars.next()?;
        if !b.is_digit(10) {
            b = chars.next()?;
        }
        let b = b.to_digit(10)?;
        if a < 5 && b < 5 {
            Some((a as u8, b as u8))
        } else {
            // This should never happen because we blacklist everything else in tesseract
            dbg!(
                "============= WARN: Found strange digits in guess {}{}",
                a,
                b
            );
            None
        }
    };

    let mut past_guess_results_found = 0usize;
    for ((x, y, _dist), past_guess_result) in past_guess_result_locations
        .into_iter()
        .zip(result.made_guesses.iter_mut())
    {
        // We make a rectangle around the comma we found to encapsulate the "X,X" text
        let (x, y) = (x - 8, y - 12);
        let (end_x, end_y) = (x + 30, y + 20);
        let (w, h) = (end_x - x, end_y - y);

        // Convert that (x,y) to screenshot coords (They were in "guess results area" fragment
        // space)
        let (test_ss_x, test_ss_y) = (x + window_x, y + window_y + SELECTOR_AREA_HEIGHT);

        let guess_result_test_image_fragment =
            &ss[(test_ss_x * 3 + test_ss_y * 3 * ss_w) as usize..];
        let title = ctx
            .ocr
            .ocr_num(
                guess_result_test_image_fragment,
                w,
                h,
                Some(core::num::NonZeroUsize::new(ss_w * 3).unwrap()),
                (x, y),
            )?
            .trim()
            .to_string();

        let guess_result = parse_guess(&title);
        if guess_result.is_none() {
            continue;
        }
        let (guess_result1, guess_result2) = guess_result?;
        // We extract a fragment rectangle to the left of the comma containing the 4 symbols
        let (xx, yy, w, h) = (
            x + window_x - 130,
            y + window_y + SELECTOR_AREA_HEIGHT - 10,
            160,
            45,
        );
        let mut guess_fragment = vec![0u8; w * h];
        for x in 0..w {
            for y in 0..h {
                let idx_out = x + y * w;
                let idx_in = (xx + x) + (yy + y) * ss_w;

                guess_fragment[idx_out] = ss_grayscale[idx_in];
            }
        }

        if ocr::ENABLE_DEBUG_IMAGE_OUTPUT {
            bmp::save_gray_bmp(
                &guess_fragment,
                w,
                h,
                &format!(
                    "dbg/guess_box_vault{}_guess{}.bmp",
                    analyzed_vault_counter, past_guess_results_found
                ),
            );
        }

        // threshold if *symbol_n == 4 { 0.15 } else { 0.11 }
        ctx.configure_finder((1, 1, 0.14), (0.5, 0.5));
        //ctx.configure_finder((1, 1, 0.11), (0.5, 0.5));
        let mut guess_symbols_x_positions = Vec::with_capacity(4);
        for (symbol_n, (smol_symbol, smol_symbol_w, smol_symbol_h)) in
            ctx.smol_symbols_gray.iter().enumerate()
        {
            let symbol_locations = ctx.finder.find_subimage_positions(
                (&guess_fragment, w, h),
                (smol_symbol, *smol_symbol_w, *smol_symbol_h),
                1,
            );
            if ocr::DEBUG_CONSOLE_OUTPUT {
                println!(
                    "guess_box_vault{}_guess{}_sym{}: {:?}",
                    analyzed_vault_counter, past_guess_results_found, symbol_n, symbol_locations
                );
            }

            for (x, _, dist) in symbol_locations {
                guess_symbols_x_positions.push((*x, symbol_n as u8, *dist));
            }
        }

        if guess_symbols_x_positions.len() < ANSWER_SIZE {
            println!(
                "============= WARN: Missing symbols in guess {}/{}",
                guess_symbols_x_positions.len(),
                ANSWER_SIZE
            );
            return None;
        }

        // If we have too many guesses keep the best 4
        if guess_symbols_x_positions.len() > ANSWER_SIZE {
            if ocr::DEBUG_CONSOLE_OUTPUT {
                println!(
                    "Too many symbols in guess {}/{} (This is fine)",
                    guess_symbols_x_positions.len(),
                    ANSWER_SIZE
                );
            }
            // Prune results that are close together, sorted by best match
            // Otherwise we can end up with something like symbol 0 and 3 for the same position
            // conflicting when we sort by x position and taking the place of another symbol.
            // This tries to ensure that for each symbol place we only have 1 symbol in the Vec, and
            // it is the best match
            guess_symbols_x_positions
                .sort_unstable_by(|(_x, _n, d), (_x2, _n2, d2)| d.partial_cmp(d2).unwrap());

            let width_threshold = 15;

            let mut i = 0;
            while i < guess_symbols_x_positions.len() {
                let a = guess_symbols_x_positions[i];

                guess_symbols_x_positions.retain(|b| {
                    let dist = (b.0 as isize - a.0 as isize).abs();
                    dist == 0 || dist > width_threshold
                });

                i += 1;
            }

            guess_symbols_x_positions.drain(ANSWER_SIZE..);
        }

        // Sort by x position to get leftmost first and store the guesses using that order
        guess_symbols_x_positions.sort_unstable_by(|(x, _n, _d), (x2, _n2, _d2)| x.cmp(x2));

        let mut guess_symbols = [0u8; ANSWER_SIZE];
        for (idx, (_, symbol_n, _)) in guess_symbols_x_positions.iter().enumerate() {
            guess_symbols[idx] = *symbol_n;
        }

        *past_guess_result = Some((guess_symbols, guess_result1, guess_result2));
        past_guess_results_found += 1;
    }

    let selection_area_h = SELECTOR_AREA_HEIGHT_ONLY_SELECTION + 30;
    let mut selection_area_fragment = vec![0u8; window_w * selection_area_h];

    let starting_in_idx = window_x + window_y * ss_w;
    for y in 0..selection_area_h {
        for x in 0..window_w {
            let out_idx = x + y * window_w;
            let in_idx = starting_in_idx + x + y * ss_w;

            selection_area_fragment[out_idx] = ss_grayscale[in_idx];
        }
    }

    if ocr::ENABLE_DEBUG_IMAGE_OUTPUT {
        bmp::save_gray_bmp(
            &selection_area_fragment,
            window_w,
            selection_area_h,
            &format!(
                "dbg/guess_box_vault{}_selectionarea.bmp",
                analyzed_vault_counter
            ),
        );
    }

    //ctx.configure_finder((1, 1, 0.05), (0.4, 0.4));
    // TODO: This threshold is way too brittle. Consider adding a bmp of the empty symbol and
    // setting that as None explicitly?
    ctx.configure_finder((1, 1, 0.090), (0.4, 0.4));
    let mut selected_symbols_x_positions = Vec::with_capacity(4);
    for (sym_n, (symbol, symbol_w, symbol_h)) in ctx.big_symbols_gray.iter().enumerate() {
        let symbol_locations = ctx.finder.find_subimage_positions(
            (&selection_area_fragment, window_w, selection_area_h),
            (symbol, *symbol_w, *symbol_h),
            1,
        );
        if ocr::DEBUG_CONSOLE_OUTPUT {
            println!(
                "guess_box_vault{}_selected_sym{}: {:?}",
                analyzed_vault_counter, sym_n, symbol_locations
            );
        }

        for (x, _, dist) in symbol_locations {
            selected_symbols_x_positions.push((*x, sym_n as u8, *dist));
        }
    }

    if selected_symbols_x_positions.len() > ANSWER_SIZE {
        if ocr::DEBUG_CONSOLE_OUTPUT {
            println!(
                "Too many symbols in guess {}/{} (This is fine)",
                selected_symbols_x_positions.len(),
                ANSWER_SIZE
            );
        }
        selected_symbols_x_positions
            .sort_unstable_by(|(_x, _n, d), (_x2, _n2, d2)| d.partial_cmp(d2).unwrap());

        let width_threshold = 15;

        let mut i = 0;
        while i < selected_symbols_x_positions.len() {
            let a = selected_symbols_x_positions[i];

            selected_symbols_x_positions.retain(|b| {
                let dist = (b.0 as isize - a.0 as isize).abs();
                dist == 0 || dist > width_threshold
            });

            i += 1;
        }

        selected_symbols_x_positions.drain(ANSWER_SIZE..);
    }

    selected_symbols_x_positions.sort_unstable_by(|(x, _n, _d), (x2, _n2, _d2)| x.cmp(x2));

    for (idx, (_, symbol_n, _)) in selected_symbols_x_positions.iter().enumerate() {
        result.selected_symbols[idx] = Some(*symbol_n);
    }

    println!("{:?}", result);
    println!("Found the vault window");

    Some((result, *left, *top))
}

fn find_minotaur_vault(
    tuple: (&[u8], usize, usize),
) -> Option<(AnalyzedMinotaurVault, usize, usize)> {
    find_minotaur_vault_impl(&mut VaultAnalyzerCtx::new().unwrap(), tuple)
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! analyze {
        ($name: expr) => {{
            let f = |(a, b, c): (Vec<u8>, _, _)| find_minotaur_vault((&a, b, c));
            f(bmp::parse_rgb_bmp(include_bytes!($name)).unwrap())
        }};
    }
    macro_rules! gen_test {
        ($test_name:ident, $file_name: expr, $selected: expr, $guess: expr, $x:expr, $y:expr) => {
            #[test]
            fn $test_name() {
                assert_eq!(
                    Some((
                        AnalyzedMinotaurVault {
                            selected_symbols: $selected,
                            made_guesses: $guess
                        },
                        $x,
                        $y
                    )),
                    analyze!($file_name)
                );
            }
        };
    }

    #[rustfmt::skip]
    gen_test!(ss1p, "ss/test1p.bmp", [Some(5), Some(11), Some(1), Some(7)],
        [Some(([0, 6, 7, 1], 1, 0)), Some(([2, 8, 9, 3], 0, 0)), Some(([4, 10, 11, 5], 1, 0)),
        Some(([0, 1, 2, 3], 0, 1)), Some(([6, 7, 8, 9], 0, 0)), Some(([2, 3, 4, 5], 0, 1)), Some(([8, 9, 10, 11], 0, 0)),
        Some(([1, 2, 3, 4], 0, 2)), Some(([7, 8, 9, 10], 0, 0)), Some(([1, 2, 9, 10], 0, 1)), Some(([7, 8, 3, 4], 0, 1))],
        1081, 217);

    #[rustfmt::skip]
    gen_test!(windowed_1, "ss/windowed_1.bmp", [Some(4), Some(10), Some(11), Some(5)],
        [Some(([0, 0, 0, 0], 0, 0)), Some(([1, 1, 1, 1], 0, 0)),
        None, None, None, None, None, None, None, None, None],
        1110, 214);

    #[test]
    fn tt1() {
        let (w, w_w, w_h) = bmp::parse_rgb_bmp(include_bytes!("ss/windowed_1.bmp")).unwrap();
        let w_g = to_grayscale(&w);

        let (window_x, window_x_w, window_x_h) =
            bmp::parse_rgb_bmp(include_bytes!("ss/windowed_gray_x.bmp"))
                .ok()
                .unwrap();
        let window_x_grayscale = to_grayscale(&window_x);

        let mut finder = find_subimage::SubImageFinderState::new();
        finder.set_backend(find_subimage::Backend::RuntimeDetectedSimd {
            step_y: 1,
            step_x: 1,
            threshold: 0.02,
        });
        finder.set_pruning(0.5, 0.5);
        let window_x_locs = finder.find_subimage_positions(
            (&w_g, w_w, w_h),
            (&window_x_grayscale, window_x_w, window_x_h),
            1,
        );
        if ocr::ENABLE_DEBUG_IMAGE_OUTPUT {
            bmp::save_gray_bmp(&w_g, w_w, w_h, "dbg/w_g.bmp");
            bmp::save_gray_bmp(
                &window_x_grayscale,
                window_x_w,
                window_x_h,
                "dbg/window_x_grayscale.bmp",
            );
        }
        if ocr::DEBUG_CONSOLE_OUTPUT {
            println!("window_x_locs: {:?}", &window_x_locs);
        }
        assert_eq!(&[(1729, 216, 0.0)], window_x_locs);

        assert_eq!(
            Some((
                AnalyzedMinotaurVault {
                    selected_symbols: [Some(4), Some(10), Some(11), Some(5)],
                    #[rustfmt::skip]
                    made_guesses: [Some(([0, 0, 0, 0], 0, 0)), Some(([1, 1, 1, 1], 0, 0)), None, None, None, None, None, None, None, None, None]
                },
                1110,
                214
            )),
            analyze!("ss/windowed_1.bmp")
        );
    }

    macro_rules! single_sym {
        ($test_name:ident, $file_name: expr, $sym_n: expr, $g1:expr, $g2: expr, $x:expr, $y:expr) => {
            #[test]
            fn $test_name() {
                assert_eq!(
                    Some((
                        AnalyzedMinotaurVault {
                            selected_symbols: [Some($sym_n); ANSWER_SIZE],
                            made_guesses: [Some(([$sym_n; 4], $g1, $g2)); MAX_GUESSES - 1]
                        },
                        $x,
                        $y
                    )),
                    analyze!($file_name)
                );
            }
        };
    }

    single_sym!(sym0p, "ss/sym0p.bmp", 0, 0, 0, 1081, 217);
    single_sym!(sym1p, "ss/sym1p.bmp", 1, 3, 0, 1081, 217);
    single_sym!(sym2p, "ss/sym2p.bmp", 2, 2, 0, 1081, 217);
    single_sym!(sym3p, "ss/sym3p.bmp", 3, 0, 0, 1081, 217);
    single_sym!(sym4p, "ss/sym4p.bmp", 4, 0, 0, 1081, 217);
    single_sym!(sym5p, "ss/sym5p.bmp", 5, 1, 0, 1081, 217);
    single_sym!(sym6p, "ss/sym6p.bmp", 6, 0, 0, 1081, 217);
    single_sym!(sym7p, "ss/sym7p.bmp", 7, 0, 0, 1081, 217);
    single_sym!(sym8p, "ss/sym8p.bmp", 8, 0, 0, 1081, 217);
    single_sym!(sym9p, "ss/sym9p.bmp", 9, 0, 0, 1081, 217);
    single_sym!(sym10p, "ss/sym10p.bmp", 10, 0, 0, 1081, 217);
    single_sym!(sym11p, "ss/sym11p.bmp", 11, 0, 0, 1081, 217);

    // All these tests below that don't end in p.bmp use lossy bmp's that went through JPG first
    // because I'm a dumb dumb and forgot to configure sharex correctly before taking them
    // Should probably remove them?
    single_sym!(sym0, "ss/sym0.bmp", 0, 2, 0, 1082, 217);
    single_sym!(sym1, "ss/sym1.bmp", 1, 0, 0, 1081, 217);
    single_sym!(sym2, "ss/sym2.bmp", 2, 1, 0, 1081, 217);
    single_sym!(sym3, "ss/sym3.bmp", 3, 3, 0, 1081, 217);
    single_sym!(sym3_2, "ss/sym3_2.bmp", 3, 3, 0, 1081, 217);
    single_sym!(sym4, "ss/sym4.bmp", 4, 0, 0, 1081, 217);
    single_sym!(sym4_2, "ss/sym4_2.bmp", 4, 0, 0, 1081, 217);

    #[rustfmt::skip]
    gen_test!(ss1, "ss/test1.bmp", [None; ANSWER_SIZE], [None; MAX_GUESSES - 1], 1062, 217);
    #[rustfmt::skip]
    gen_test!(ss2, "ss/test2.bmp", [Some(1), Some(1), Some(0), Some(0)], [None; MAX_GUESSES - 1], 1075, 217);
    #[rustfmt::skip]
    gen_test!(ss3, "ss/test3.bmp", [None; ANSWER_SIZE],
        [Some(([0, 0, 1, 1], 2, 0)), None, None, None, None, None, None, None, None, None, None],
        1071, 217);
    #[rustfmt::skip]
    gen_test!(ss4, "ss/test4.bmp", [None; ANSWER_SIZE],
        [Some(([0, 0, 1, 1], 2, 0)), Some(([3, 3, 4, 4], 1, 0)), Some(([0, 0, 4, 0], 2, 0)), Some(([2, 0, 4, 2], 1, 1)),
        Some(([1, 0, 0, 0], 0, 2)), Some(([4, 4, 2, 2], 0, 1)), Some(([6, 6, 7, 7], 0, 0)), Some(([8, 8, 9, 9], 0, 0)),
        Some(([11, 11, 10, 10], 0, 0)), None, None],
        1061, 217);
    #[rustfmt::skip]
    gen_test!(ss5, "ss/test5.bmp", [Some(0), Some(3), Some(4), Some(1)],
        [Some(([0, 0, 1, 1], 2, 0)), Some(([3, 3, 4, 4], 1, 0)), Some(([0, 0, 4, 0], 2, 0)),
        Some(([2, 0, 4, 2], 1, 1)), Some(([1, 0, 0, 0], 0, 2)), Some(([4, 4, 2, 2], 0, 1)), Some(([6, 6, 7, 7], 0, 0)),
        Some(([8, 8, 9, 9], 0, 0)), Some(([11, 11, 10, 10], 0, 0)), None, None],
        1053, 217);
    #[rustfmt::skip]
    gen_test!(ss6, "ss/test6.bmp", [Some(0), Some(5), Some(4), Some(1)],
        [Some(([0, 0, 1, 1], 2, 0)), Some(([3, 3, 4, 4], 1, 0)), Some(([0, 0, 4, 0], 2, 0)),
        Some(([2, 0, 4, 2], 1, 1)), Some(([1, 0, 0, 0], 0, 2)), Some(([4, 4, 2, 2], 0, 1)), Some(([6, 6, 7, 7], 0, 0)),
        Some(([8, 8, 9, 9], 0, 0)), Some(([11, 11, 10, 10], 0, 0)), Some(([0, 3, 4, 1], 3, 0)), None],
        1053, 217);
}

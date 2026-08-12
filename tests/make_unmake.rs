use neyrang::chess::Position;

#[test]
fn randomized_legal_sequences_restore_the_exact_starting_state() {
    for seed in 1..=64_u64 {
        let mut random = SplitMix64(seed);
        let mut position = Position::startpos();
        let original = position.clone();
        let mut played = Vec::with_capacity(96);

        for _ in 0..96 {
            let moves = position.legal_moves();
            if moves.is_empty() {
                break;
            }
            let mv = moves.as_slice()[(random.next() as usize) % moves.len()];
            let undo = position.make_move(mv);
            position
                .verify_integrity()
                .unwrap_or_else(|error| panic!("seed {seed}, move {mv}: {error}"));
            played.push((mv, undo));
        }

        while let Some((mv, undo)) = played.pop() {
            position.unmake_move(mv, undo);
            position
                .verify_integrity()
                .unwrap_or_else(|error| panic!("seed {seed}, unmove {mv}: {error}"));
        }
        assert_eq!(position, original, "seed {seed}");
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }
}

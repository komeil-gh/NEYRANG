use super::{
    Bitboard, Color, FenError, Move, MoveFlag, MoveList, NullUndoState, Piece, PieceType, Square,
    UndoState, attacks, zobrist,
};

/// Castling flags stored as KQkq bits.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CastlingRights(u8);

impl CastlingRights {
    pub const WHITE_KING: u8 = 1;
    pub const WHITE_QUEEN: u8 = 2;
    pub const BLACK_KING: u8 = 4;
    pub const BLACK_QUEEN: u8 = 8;

    #[inline]
    pub(crate) const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    #[inline]
    pub const fn contains(self, right: u8) -> bool {
        self.0 & right != 0
    }

    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[inline]
    pub(crate) fn remove(&mut self, rights: u8) {
        self.0 &= !rights;
    }
}

/// Complete chess position. Bitboards are authoritative and the mailbox makes
/// captures and FEN serialization inexpensive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    pub(crate) pieces: [[Bitboard; 6]; 2],
    pub(crate) occupancy: [Bitboard; 2],
    pub(crate) all_occupancy: Bitboard,
    pub(crate) mailbox: [Option<Piece>; 64],
    pub(crate) side_to_move: Color,
    pub(crate) castling_rights: CastlingRights,
    pub(crate) en_passant: Option<Square>,
    pub(crate) halfmove_clock: u16,
    pub(crate) fullmove_number: u16,
    pub(crate) king_squares: [Option<Square>; 2],
    pub(crate) hash: u64,
}

impl Position {
    pub const STARTPOS_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    pub fn startpos() -> Self {
        Self::from_fen(Self::STARTPOS_FEN).expect("the built-in start position must be valid")
    }

    pub(crate) const fn empty() -> Self {
        Self {
            pieces: [[0; 6]; 2],
            occupancy: [0; 2],
            all_occupancy: 0,
            mailbox: [None; 64],
            side_to_move: Color::White,
            castling_rights: CastlingRights::from_bits(0),
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            king_squares: [None; 2],
            hash: 0,
        }
    }

    #[inline]
    pub const fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    #[inline]
    pub const fn en_passant(&self) -> Option<Square> {
        self.en_passant
    }

    #[inline]
    pub const fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }

    #[inline]
    pub const fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    #[inline]
    pub fn piece_at(&self, square: Square) -> Option<(Color, PieceType)> {
        self.mailbox[square.index()].map(|piece| (piece.color, piece.kind))
    }

    #[inline]
    pub const fn pieces(&self, color: Color, kind: PieceType) -> Bitboard {
        self.pieces[color.index()][kind.index()]
    }

    #[inline]
    pub const fn occupancy(&self, color: Color) -> Bitboard {
        self.occupancy[color.index()]
    }

    #[inline]
    pub const fn all_occupancy(&self) -> Bitboard {
        self.all_occupancy
    }

    #[inline]
    pub const fn castling_rights(&self) -> CastlingRights {
        self.castling_rights
    }

    #[inline]
    pub const fn hash(&self) -> u64 {
        self.hash
    }

    /// Return the position key used for repetition detection.
    ///
    /// The transposition hash retains the FEN en-passant field verbatim. For
    /// repetition, that field distinguishes positions only when the side to
    /// move can legally capture en passant.
    pub fn repetition_hash(&mut self) -> u64 {
        let Some(target) = self.en_passant else {
            return self.hash;
        };
        if self.has_legal_en_passant_capture(target) {
            self.hash
        } else {
            self.hash ^ zobrist::en_passant_key(target.file())
        }
    }

    fn has_legal_en_passant_capture(&mut self, target: Square) -> bool {
        let moving_color = self.side_to_move;
        let expected_rank = match moving_color {
            Color::White => 5,
            Color::Black => 2,
        };
        if target.rank() != expected_rank || self.piece_at(target).is_some() {
            return false;
        }
        let mut candidates = attacks::pawn_attacks(moving_color.opposite(), target)
            & self.pieces(moving_color, PieceType::Pawn);
        while candidates != 0 {
            let index = candidates.trailing_zeros() as u8;
            candidates &= candidates - 1;
            let from = Square::from_index(index).expect("a pawn bit identifies a valid square");
            let captured_square = Square::from_coords(target.file(), from.rank())
                .expect("an en-passant capture square is on the board");
            if self.piece_at(captured_square) != Some((moving_color.opposite(), PieceType::Pawn)) {
                continue;
            }

            let mv = Move::new(from, target, MoveFlag::EnPassant);
            let undo = self.make_move(mv);
            let legal = !self.is_in_check(moving_color);
            self.unmake_move(mv, undo);
            if legal {
                return true;
            }
        }
        false
    }

    pub(crate) fn place_piece(&mut self, square: Square, piece: Piece) {
        let bit = square.bit();
        self.pieces[piece.color.index()][piece.kind.index()] |= bit;
        self.occupancy[piece.color.index()] |= bit;
        self.all_occupancy |= bit;
        self.mailbox[square.index()] = Some(piece);
        if piece.kind == PieceType::King {
            self.king_squares[piece.color.index()] = Some(square);
        }
    }

    pub(crate) fn remove_piece(&mut self, square: Square) -> Option<Piece> {
        let piece = self.mailbox[square.index()]?;
        let bit = square.bit();
        self.pieces[piece.color.index()][piece.kind.index()] &= !bit;
        self.occupancy[piece.color.index()] &= !bit;
        self.all_occupancy &= !bit;
        self.mailbox[square.index()] = None;
        if piece.kind == PieceType::King {
            self.king_squares[piece.color.index()] = None;
        }
        Some(piece)
    }

    fn move_piece(&mut self, from: Square, to: Square) -> Piece {
        let piece = self
            .remove_piece(from)
            .expect("internal move source must contain a piece");
        self.place_piece(to, piece);
        piece
    }

    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        super::fen::parse(fen)
    }

    pub fn to_fen(&self) -> String {
        super::fen::serialize(self)
    }

    pub fn legal_moves(&mut self) -> MoveList {
        super::movegen::generate_legal(self)
    }

    pub fn is_in_check(&self, color: Color) -> bool {
        super::movegen::is_in_check(self, color)
    }

    pub fn find_legal_move(&mut self, notation: &str) -> Option<Move> {
        self.legal_moves()
            .iter()
            .copied()
            .find(|mv| mv.to_string() == notation)
    }

    /// Slow reference check for tests and debug assertions, never used in the
    /// tournament hot path.
    pub fn verify_integrity(&self) -> Result<(), String> {
        let mut pieces = [[0_u64; 6]; 2];
        let mut occupancy = [0_u64; 2];
        let mut kings = [None; 2];
        for (index, piece) in self.mailbox.iter().copied().enumerate() {
            let Some(piece) = piece else { continue };
            let square = Square::from_index(index as u8)
                .ok_or_else(|| "mailbox index is outside the board".to_owned())?;
            pieces[piece.color.index()][piece.kind.index()] |= square.bit();
            occupancy[piece.color.index()] |= square.bit();
            if piece.kind == PieceType::King && kings[piece.color.index()].replace(square).is_some()
            {
                return Err("a side has multiple kings".to_owned());
            }
        }
        if pieces != self.pieces {
            return Err("piece bitboards disagree with the mailbox".to_owned());
        }
        if occupancy != self.occupancy {
            return Err("color occupancy disagrees with the mailbox".to_owned());
        }
        if occupancy[0] & occupancy[1] != 0 {
            return Err("white and black occupancy overlap".to_owned());
        }
        if occupancy[0] | occupancy[1] != self.all_occupancy {
            return Err("combined occupancy is stale".to_owned());
        }
        if kings != self.king_squares || kings.iter().any(Option::is_none) {
            return Err("cached king squares are invalid".to_owned());
        }
        if self.hash != zobrist::recompute(self) {
            return Err("incremental Zobrist hash differs from recomputation".to_owned());
        }
        Ok(())
    }

    /// Apply a move already validated by legal move generation and return the
    /// exact irreversible state required to restore the parent position.
    pub fn make_move(&mut self, mv: Move) -> UndoState {
        let from = mv.from();
        let to = mv.to();
        let moving_piece = self
            .remove_piece(from)
            .expect("a generated move must have a piece on its source square");
        let mut next_hash = self.hash ^ zobrist::piece_key(moving_piece, from);
        next_hash ^= zobrist::castling_key(self.castling_rights.bits());
        if let Some(square) = self.en_passant {
            next_hash ^= zobrist::en_passant_key(square.file());
        }

        let capture_square = if mv.is_en_passant() {
            Square::from_coords(to.file(), from.rank())
                .expect("en-passant capture square must be on the board")
        } else {
            to
        };
        let captured = if mv.is_capture() {
            self.remove_piece(capture_square).map(|piece| {
                next_hash ^= zobrist::piece_key(piece, capture_square);
                (capture_square, piece)
            })
        } else {
            None
        };

        let undo = UndoState::new(
            captured,
            self.castling_rights,
            self.en_passant,
            self.halfmove_clock,
            self.fullmove_number,
            self.hash,
        );

        if mv.is_castle() {
            let (rook_from, rook_to) = castle_rook_squares(moving_piece.color, mv.flag());
            let rook = self.move_piece(rook_from, rook_to);
            next_hash ^= zobrist::piece_key(rook, rook_from) ^ zobrist::piece_key(rook, rook_to);
        }

        let placed_piece = Piece {
            color: moving_piece.color,
            kind: mv.promotion().unwrap_or(moving_piece.kind),
        };
        self.place_piece(to, placed_piece);
        next_hash ^= zobrist::piece_key(placed_piece, to);

        self.update_castling_rights(from, moving_piece, captured);
        self.en_passant = if mv.is_double_pawn_push() {
            Square::from_coords(from.file(), (from.rank() + to.rank()) / 2)
        } else {
            None
        };
        if moving_piece.kind == PieceType::Pawn || captured.is_some() {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        }
        if self.side_to_move == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }
        self.side_to_move = self.side_to_move.opposite();

        next_hash ^= zobrist::castling_key(self.castling_rights.bits());
        if let Some(square) = self.en_passant {
            next_hash ^= zobrist::en_passant_key(square.file());
        }
        next_hash ^= zobrist::side_key();
        self.hash = next_hash;

        debug_assert_eq!(self.hash, zobrist::recompute(self));
        undo
    }

    pub fn unmake_move(&mut self, mv: Move, undo: UndoState) {
        self.side_to_move = self.side_to_move.opposite();
        let from = mv.from();
        let to = mv.to();
        let moved_piece = self
            .remove_piece(to)
            .expect("unmade move must have a piece on its destination square");

        if mv.is_castle() {
            let (rook_from, rook_to) = castle_rook_squares(self.side_to_move, mv.flag());
            self.move_piece(rook_to, rook_from);
        }

        self.place_piece(
            from,
            Piece {
                color: self.side_to_move,
                kind: if mv.is_promotion() {
                    PieceType::Pawn
                } else {
                    moved_piece.kind
                },
            },
        );
        if let Some((square, piece)) = undo.captured() {
            self.place_piece(square, piece);
        }

        self.castling_rights = undo.castling_rights();
        self.en_passant = undo.en_passant();
        self.halfmove_clock = undo.halfmove_clock();
        self.fullmove_number = undo.fullmove_number();
        self.hash = undo.hash();
        debug_assert_eq!(self.hash, zobrist::recompute(self));
    }

    /// Pass the turn for a search-only null probe without creating a chess
    /// move. Pieces, castling rights, and game clocks remain untouched.
    pub fn make_null_move(&mut self) -> NullUndoState {
        let undo = NullUndoState::new(self.en_passant, self.hash);
        if let Some(square) = self.en_passant {
            self.hash ^= zobrist::en_passant_key(square.file());
        }
        self.en_passant = None;
        self.side_to_move = self.side_to_move.opposite();
        self.hash ^= zobrist::side_key();

        debug_assert_eq!(self.hash, zobrist::recompute(self));
        undo
    }

    /// Restore the exact state saved by [`Position::make_null_move`].
    pub fn unmake_null_move(&mut self, undo: NullUndoState) {
        self.side_to_move = self.side_to_move.opposite();
        self.en_passant = undo.en_passant();
        self.hash = undo.hash();

        debug_assert_eq!(self.hash, zobrist::recompute(self));
    }

    fn update_castling_rights(
        &mut self,
        from: Square,
        moving_piece: Piece,
        captured: Option<(Square, Piece)>,
    ) {
        if moving_piece.kind == PieceType::King {
            self.castling_rights.remove(match moving_piece.color {
                Color::White => CastlingRights::WHITE_KING | CastlingRights::WHITE_QUEEN,
                Color::Black => CastlingRights::BLACK_KING | CastlingRights::BLACK_QUEEN,
            });
        }
        if moving_piece.kind == PieceType::Rook {
            self.castling_rights.remove(rook_castling_right(from));
        }
        if let Some((square, piece)) = captured
            && piece.kind == PieceType::Rook
        {
            self.castling_rights.remove(rook_castling_right(square));
        }
    }
}

fn rook_castling_right(square: Square) -> u8 {
    match square {
        Square::A1 => CastlingRights::WHITE_QUEEN,
        Square::H1 => CastlingRights::WHITE_KING,
        Square::A8 => CastlingRights::BLACK_QUEEN,
        Square::H8 => CastlingRights::BLACK_KING,
        _ => 0,
    }
}

fn castle_rook_squares(color: Color, flag: MoveFlag) -> (Square, Square) {
    match (color, flag) {
        (Color::White, MoveFlag::KingCastle) => (Square::H1, Square::F1),
        (Color::White, MoveFlag::QueenCastle) => (Square::A1, Square::D1),
        (Color::Black, MoveFlag::KingCastle) => (Square::H8, Square::F8),
        (Color::Black, MoveFlag::QueenCastle) => (Square::A8, Square::D8),
        _ => unreachable!("only castling moves relocate a rook"),
    }
}

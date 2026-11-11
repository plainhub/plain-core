macro_rules ! decoder_function { ($ preamble : block , $ loop_preamble : block , $ eof : block , $ body : block , $ slf : ident , $ src_consumed : ident , $ dest : ident , $ source : ident , $ b : ident , $ destination_handle : ident , $ unread_handle : ident , $ destination_check : ident , $ name : ident , $ code_unit : ty , $ dest_struct : ident) => (# [inline (always)] pub fn $ name (& mut $ slf , src : & [u8] , dst : & mut [$ code_unit] , last : bool) -> (DecoderResult , usize , usize) { let mut $ source = ByteSource :: new (src) ; let mut $ dest = $ dest_struct :: new (dst) ; loop { { $ preamble } loop { { $ loop_preamble } match $ source . check_available () { Space :: Full ($ src_consumed) => { if last { $ eof } return (DecoderResult :: InputEmpty , $ src_consumed , $ dest . written ()) ; } Space :: Available (source_handle) => { match $ dest .$ destination_check () { Space :: Full (dst_written) => { return (DecoderResult :: OutputFull , source_handle . consumed () , dst_written) ; } Space :: Available ($ destination_handle) => { let ($ b , $ unread_handle) = source_handle . read () ; $ body } } } } } } }) ; }
macro_rules! decoder_functions {
    (preamble = $ preamble : block , loop_preamble = $ loop_preamble : block , eof = $ eof : block , body = $ body : block , self = $ slf : ident , src_consumed = $ src_consumed : ident , dest = $ dest : ident , source = $ source : ident , byte = $ b : ident , destination_handle = $ destination_handle : ident , unread_handle = $ unread_handle : ident , destination_check = $ destination_check : ident) => {
        decoder_function!(
            $preamble,
            $loop_preamble,
            $eof,
            $body,
            $slf,
            $src_consumed,
            $dest,
            $source,
            $b,
            $destination_handle,
            $unread_handle,
            $destination_check,
            decode_to_utf8_raw,
            u8,
            Utf8Destination
        );
    };
}
macro_rules ! ascii_compatible_two_byte_decoder_function { ($ lead : block , $ trail : block , $ slf : ident , $ non_ascii : ident , $ byte : ident , $ lead_minus_offset : ident , $ unread_handle_trail : ident , $ source : ident , $ handle : ident , $ outermost : tt , $ copy_ascii : ident , $ destination_check : ident , $ name : ident , $ code_unit : ty , $ dest_struct : ident , $ ascii_punctuation : expr) => (# [inline (always)] pub fn $ name (& mut $ slf , src : & [u8] , dst : & mut [$ code_unit] , last : bool) -> (DecoderResult , usize , usize) { let mut $ source = ByteSource :: new (src) ; let mut dest_prolog = $ dest_struct :: new (dst) ; let dest = match $ slf . lead { Some (lead) => { let $ lead_minus_offset = lead ; match $ source . check_available () { Space :: Full (src_consumed_prolog) => { if last { $ slf . lead = None ; return (DecoderResult :: Malformed (1 , 0) , src_consumed_prolog , dest_prolog . written ()) ; } return (DecoderResult :: InputEmpty , src_consumed_prolog , dest_prolog . written ()) ; } Space :: Available (source_handle_prolog) => { match dest_prolog .$ destination_check () { Space :: Full (dst_written_prolog) => { return (DecoderResult :: OutputFull , source_handle_prolog . consumed () , dst_written_prolog) ; } Space :: Available ($ handle) => { $ slf . lead = None ; let ($ byte , $ unread_handle_trail) = source_handle_prolog . read () ; $ trail } } } } } , None => { & mut dest_prolog } } ; $ outermost : loop { match dest .$ copy_ascii (& mut $ source) { CopyAsciiResult :: Stop (ret) => return ret , CopyAsciiResult :: GoOn ((mut $ non_ascii , mut $ handle)) => { 'middle : loop { let dest_again = { let $ lead_minus_offset = { $ lead } ; match $ source . check_available () { Space :: Full (src_consumed_trail) => { if last { return (DecoderResult :: Malformed (1 , 0) , src_consumed_trail , $ handle . written ()) ; } $ slf . lead = Some ($ lead_minus_offset) ; return (DecoderResult :: InputEmpty , src_consumed_trail , $ handle . written ()) ; } Space :: Available (source_handle_trail) => { let ($ byte , $ unread_handle_trail) = source_handle_trail . read () ; $ trail } } } ; match $ source . check_available () { Space :: Full (src_consumed) => { return (DecoderResult :: InputEmpty , src_consumed , dest_again . written ()) ; } Space :: Available (source_handle) => { match dest_again .$ destination_check () { Space :: Full (dst_written) => { return (DecoderResult :: OutputFull , source_handle . consumed () , dst_written) ; } Space :: Available (mut destination_handle) => { let (mut b , unread_handle) = source_handle . read () ; let source_again = unread_handle . commit () ; 'innermost : loop { if b > 127 { $ non_ascii = b ; $ handle = destination_handle ; continue 'middle ; } let dest_again_again = destination_handle . write_ascii (b) ; if $ ascii_punctuation && b < 60 { match source_again . check_available () { Space :: Full (src_consumed_again) => { return (DecoderResult :: InputEmpty , src_consumed_again , dest_again_again . written ()) ; } Space :: Available (source_handle_again) => { match dest_again_again .$ destination_check () { Space :: Full (dst_written_again) => { return (DecoderResult :: OutputFull , source_handle_again . consumed () , dst_written_again) ; } Space :: Available (destination_handle_again) => { { let (b_again , unread_handle_again) = source_handle_again . read () ; unread_handle_again . commit () ; b = b_again ; destination_handle = destination_handle_again ; continue 'innermost ; } } } } } } continue $ outermost ; } } } } } } } } } }) ; }
macro_rules! ascii_compatible_two_byte_decoder_functions {
    (lead = $ lead : block , trail = $ trail : block , self = $ slf : ident , non_ascii = $ non_ascii : ident , byte = $ byte : ident , lead_minus_offset = $ lead_minus_offset : ident , unread_handle_trail = $ unread_handle_trail : ident , source = $ source : ident , handle = $ handle : ident , outermost = $ outermost : tt , copy_ascii = $ copy_ascii : ident , destination_check = $ destination_check : ident , ascii_punctuation = $ ascii_punctuation : expr) => {
        ascii_compatible_two_byte_decoder_function!(
            $lead,
            $trail,
            $slf,
            $non_ascii,
            $byte,
            $lead_minus_offset,
            $unread_handle_trail,
            $source,
            $handle,
            $outermost,
            $copy_ascii,
            $destination_check,
            decode_to_utf8_raw,
            u8,
            Utf8Destination,
            $ascii_punctuation
        );
    };
}
macro_rules ! gb18030_decoder_function { ($ first_body : block , $ second_body : block , $ third_body : block , $ fourth_body : block , $ slf : ident , $ non_ascii : ident , $ first_minus_offset : ident , $ second : ident , $ second_minus_offset : ident , $ unread_handle_second : ident , $ third : ident , $ third_minus_offset : ident , $ unread_handle_third : ident , $ fourth : ident , $ fourth_minus_offset : ident , $ unread_handle_fourth : ident , $ source : ident , $ handle : ident , $ outermost : tt , $ name : ident , $ code_unit : ty , $ dest_struct : ident) => (# [inline (always)] pub fn $ name (& mut $ slf , src : & [u8] , dst : & mut [$ code_unit] , last : bool) -> (DecoderResult , usize , usize) { let mut $ source = ByteSource :: new (src) ; let mut dest = $ dest_struct :: new (dst) ; { if let Some (ascii) = $ slf . pending_ascii { match dest . check_space_bmp () { Space :: Full (_) => { return (DecoderResult :: OutputFull , 0 , 0) ; } Space :: Available (pending_ascii_handle) => { $ slf . pending_ascii = None ; pending_ascii_handle . write_ascii (ascii) ; } } } } while !$ slf . pending . is_none () { match $ source . check_available () { Space :: Full (src_consumed) => { if last { let count = $ slf . pending . count () ; $ slf . pending = Gb18030Pending :: None ; return (DecoderResult :: Malformed (count as u8 , 0) , src_consumed , dest . written ()) ; } return (DecoderResult :: InputEmpty , src_consumed , dest . written ()) ; } Space :: Available (source_handle) => { match dest . check_space_astral () { Space :: Full (dst_written) => { return (DecoderResult :: OutputFull , source_handle . consumed () , dst_written) ; } Space :: Available ($ handle) => { let (byte , unread_handle) = source_handle . read () ; match $ slf . pending { Gb18030Pending :: One ($ first_minus_offset) => { $ slf . pending = Gb18030Pending :: None ; let $ second = byte ; let $ unread_handle_second = unread_handle ; let $ second_minus_offset = $ second . wrapping_sub (0x30) ; if $ second_minus_offset > (0x39 - 0x30) { $ second_body } else { $ slf . pending = Gb18030Pending :: Two ($ first_minus_offset , $ second_minus_offset) ; $ handle . commit () } } Gb18030Pending :: Two ($ first_minus_offset , $ second_minus_offset) => { $ slf . pending = Gb18030Pending :: None ; let $ third = byte ; let $ unread_handle_third = unread_handle ; let $ third_minus_offset = { $ third_body } ; $ slf . pending = Gb18030Pending :: Three ($ first_minus_offset , $ second_minus_offset , $ third_minus_offset) ; $ handle . commit () } Gb18030Pending :: Three ($ first_minus_offset , $ second_minus_offset , $ third_minus_offset) => { $ slf . pending = Gb18030Pending :: None ; let $ fourth = byte ; let $ unread_handle_fourth = unread_handle ; $ fourth_body } Gb18030Pending :: None => unreachable ! ("Checked in loop condition") , } ; } } } } } $ outermost : loop { match dest . copy_ascii_from_check_space_astral (& mut $ source) { CopyAsciiResult :: Stop (ret) => return ret , CopyAsciiResult :: GoOn ((mut $ non_ascii , mut $ handle)) => { 'middle : loop { let dest_again = { let $ first_minus_offset = { $ first_body } ; match $ source . check_available () { Space :: Full (src_consumed_trail) => { if last { return (DecoderResult :: Malformed (1 , 0) , src_consumed_trail , $ handle . written ()) ; } $ slf . pending = Gb18030Pending :: One ($ first_minus_offset) ; return (DecoderResult :: InputEmpty , src_consumed_trail , $ handle . written ()) ; } Space :: Available (source_handle_trail) => { let ($ second , $ unread_handle_second) = source_handle_trail . read () ; let $ second_minus_offset = $ second . wrapping_sub (0x30) ; if $ second_minus_offset > (0x39 - 0x30) { $ second_body } else { match $ unread_handle_second . commit () . check_available () { Space :: Full (src_consumed_third) => { if last { return (DecoderResult :: Malformed (2 , 0) , src_consumed_third , $ handle . written ()) ; } $ slf . pending = Gb18030Pending :: Two ($ first_minus_offset , $ second_minus_offset) ; return (DecoderResult :: InputEmpty , src_consumed_third , $ handle . written ()) ; } Space :: Available (source_handle_third) => { let ($ third , $ unread_handle_third) = source_handle_third . read () ; let $ third_minus_offset = { $ third_body } ; match $ unread_handle_third . commit () . check_available () { Space :: Full (src_consumed_fourth) => { if last { return (DecoderResult :: Malformed (3 , 0) , src_consumed_fourth , $ handle . written ()) ; } $ slf . pending = Gb18030Pending :: Three ($ first_minus_offset , $ second_minus_offset , $ third_minus_offset) ; return (DecoderResult :: InputEmpty , src_consumed_fourth , $ handle . written ()) ; } Space :: Available (source_handle_fourth) => { let ($ fourth , $ unread_handle_fourth) = source_handle_fourth . read () ; $ fourth_body } } } } } } } } ; match $ source . check_available () { Space :: Full (src_consumed) => { return (DecoderResult :: InputEmpty , src_consumed , dest_again . written ()) ; } Space :: Available (source_handle) => { match dest_again . check_space_astral () { Space :: Full (dst_written) => { return (DecoderResult :: OutputFull , source_handle . consumed () , dst_written) ; } Space :: Available (destination_handle) => { let (b , unread_handle) = source_handle . read () ; unread_handle . commit () ; loop { if b > 127 { $ non_ascii = b ; $ handle = destination_handle ; continue 'middle ; } destination_handle . write_ascii (b) ; continue $ outermost ; } } } } } } } } } }) ; }
macro_rules! gb18030_decoder_functions {
    ($ first_body : block , $ second_body : block , $ third_body : block , $ fourth_body : block , $ slf : ident , $ non_ascii : ident , $ first_minus_offset : ident , $ second : ident , $ second_minus_offset : ident , $ unread_handle_second : ident , $ third : ident , $ third_minus_offset : ident , $ unread_handle_third : ident , $ fourth : ident , $ fourth_minus_offset : ident , $ unread_handle_fourth : ident , $ source : ident , $ handle : ident , $ outermost : tt) => {
        gb18030_decoder_function!(
            $first_body,
            $second_body,
            $third_body,
            $fourth_body,
            $slf,
            $non_ascii,
            $first_minus_offset,
            $second,
            $second_minus_offset,
            $unread_handle_second,
            $third,
            $third_minus_offset,
            $unread_handle_third,
            $fourth,
            $fourth_minus_offset,
            $unread_handle_fourth,
            $source,
            $handle,
            $outermost,
            decode_to_utf8_raw,
            u8,
            Utf8Destination
        );
    };
}

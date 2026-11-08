#!/usr/bin/env python3
from base64 import b64encode
from pathlib import Path
from struct import pack
from zlib import compress, crc32


def syncsafe(value):
    return bytes((value >> shift) & 127 for shift in (21, 14, 7, 0))


def id3(version, frames):
    payload = bytearray()
    for name, data in frames:
        payload.extend(name)
        if version == 2:
            payload.extend(len(data).to_bytes(3, "big"))
        else:
            payload.extend(syncsafe(len(data)) if version == 4 else pack(">I", len(data)))
            payload.extend(b"\0\0")
        payload.extend(data)
    return b"ID3" + bytes((version, 0, 0)) + syncsafe(len(payload)) + payload


def utf16(text):
    return b"\xff\xfe" + text.encode("utf-16le")


def box(kind, data):
    return pack(">I", 8 + len(data)) + kind + data


def atom(kind, *values):
    return box(kind, b"".join(box(b"data", pack(">II", code, 0) + data) for code, data in values))


def chunk(kind, data, endian):
    return kind + pack(endian + "I", len(data)) + data + b"\0" * (len(data) % 2)


def vorbis(vendor, *comments):
    def sized(data):
        return pack("<I", len(data)) + data

    return sized(vendor) + pack("<I", len(comments)) + b"".join(map(sized, comments))


def fixtures():
    title, artist, album, comment, year, track, genre = (
        b"Foo title", b"Bar artist", b"Baz album", b"Qux comment", b"1984", b"1", b"Classical"
    )
    ape_items = [
        (b"TITLE", title), (b"ARTIST", artist), (b"ALBUM", album), (b"COMMENT", comment),
        (b"YEAR", year), (b"TRACK", track), (b"GENRE", genre),
    ]
    ape_payload = b"".join(pack("<II", len(value), 0) + key + b"\0" + value for key, value in ape_items)
    ape_header = b"APETAGEX" + pack("<III", 2000, len(ape_payload) + 32, len(ape_items))
    yield "test.apev2", ape_header + pack("<I", 0xA0000000) + bytes(8) + ape_payload + ape_header + pack("<I", 0x80000000) + bytes(8)

    yield "test.id3v1", pack("3s30s30s30s4s28s3B", b"TAG", title, artist, album, year, comment, 0, 1, 32)

    riff = [(b"IART", artist), (b"ICMT", comment), (b"ICRD", year), (b"INAM", title), (b"IPRD", album), (b"IPRT", track)]
    yield "test.riff", b"".join(chunk(key, value + b"\0", "<") for key, value in riff)

    aiff = [(b"NAME", title), (b"AUTH", artist), (b"(c) ", b"Baz copyright"), (b"ANNO", b"Qux annotation"), (b"ANNO", b"Quux annotation")]
    comments = pack(">H", 2)
    for timestamp, marker, text in [(1024, 0, b"Quuz comment"), (2048, 40, b"Corge comment")]:
        comments += pack(">IHH", timestamp, marker, len(text)) + text + b"\0" * (len(text) % 2)
    yield "test.aiff_text", chunk(b"FORM", b"AIFF" + b"".join(chunk(key, value, ">") for key, value in aiff + [(b"COMT", comments)]), ">")
    yield "zero.aiff_text", chunk(b"FORM", b"AIFF" + b"".join(chunk(key, b"", ">") for key in (b"NAME", b"AUTH", b"(c) ", b"ANNO", b"COMT")), ">")

    yield "test.vorbis", vorbis(b"Fixture vendor", b"ALBUM=" + album, b"ARTIST=" + artist,
                               b"COMMENT=" + comment, b"DATE=" + year, b"GENRE=" + genre,
                               b"TITLE=" + title, b"TRACKNUMBER=" + track)
    yield "zero.vorbis", vorbis(b"")

    def png_chunk(kind, data):
        return pack(">I", len(data)) + kind + data + pack(">I", crc32(kind + data))

    png = (b"\x89PNG\r\n\x1a\n" + png_chunk(b"IHDR", pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0))
           + png_chunk(b"IDAT", compress(b"\0\xff\x80\x00")) + png_chunk(b"IEND", b""))
    picture = pack(">II", 3, 9) + b"image/png" + pack(">IIIIII", 0, 1, 1, 24, 0, len(png)) + png
    yield "issue_60.vorbis", vorbis(b"", b"", b"METADATA_BLOCK_PICTURE=" + b64encode(picture))
    yield "id3v2/test_full_cover.png", png

    frames = [(b"TPE1", b"\0" + artist), (b"TIT2", b"\0" + title), (b"TALB", b"\0" + album),
              (b"COMM", b"\0eng\0" + comment), (b"TDRC", b"\0" + year), (b"TRCK", b"\0" + track), (b"TCON", b"\0" + genre)]
    yield "id3v2/test.id3v24", id3(4, frames)
    yield "id3v2/zero.id3v2", id3(4, [])
    yield "id3v2/bad_frame_otherwise_valid.id3v24", id3(4, frames[:2] + [(b"T+XX", b"\0bad")] + frames[3:])
    yield "id3v2/skippable_frame_otherwise_valid.id3v24", id3(4, [(b"TDEN", b"\0\xffinvalid"), (b"TDTG", b"\0" + b"2014-06-10T02:16:10")])
    yield "id3v2/multiple_composers.id3v24", id3(4, [(b"TCOM", b"\x03A\0B")])
    yield "id3v2/issue_53.id3v24", id3(4, [(b"TXXX", b"\x01" + utf16("Description\0Value"))])
    yield "id3v2/trailing_nulls.id3v24", id3(4, [
        (b"TPE1", b"\x03Artist\0"), (b"TXXX", b"\x03Writer\0Writer\0"),
        (b"USLT", b"\x03eng\0Lyrics to the song\0"), (b"COMM", b"\x03eng\0Comment\0"),
        (b"WXXX", b"\x03\0https://example.invalid/artist\0"),
    ])

    for version in (2, 3, 4):
        date_frame = b"TDRC" if version == 4 else b"TYER"
        text_frames = [
            (b"TIT2", "Fixture Track"), (b"TPE1", "Fixture Artist"), (b"TRCK", "1"),
            (b"TALB", "Fixture Album"), (date_frame, "2017"), (b"TCON", "Electronic"), (b"TLEN", "213017"),
        ]
        if version == 2:
            names = dict(zip((name for name, _ in text_frames), (b"TT2", b"TP1", b"TRK", b"TAL", b"TYE", b"TCO", b"TLE")))
            text_frames = [(names[name], value) for name, value in text_frames]
        picture_frame = (b"PIC", b"\0PNG\x03\0" + png) if version == 2 else (b"APIC", b"\0image/png\0\x03\0" + png)
        yield f"id3v2/test_full.id3v2{version}", id3(version, [(name, b"\x01" + utf16(text)) for name, text in text_frames] + [picture_frame])

    audio_payload = b"\xff\xfb\x90\x64\x00\x7f\x80\xff"
    yield "id3v2/test.atxt", b"\0audio/mpeg\0\0foo bar baz\0" + audio_payload
    yield "id3v2/test.geob", b"\0audio/mpeg\0a.mp3\0Test Asset\0" + audio_payload
    yield "id3v2/test.owne", b"\0USD1000\0" + b"19840407FooBar"
    yield "id3v2/test.priv", b"unit\x40x.test\0some data"
    yield "id3v2/test.etco", b"\x02" + b"".join(pack(">BI", code, ms) for code, ms in [
        (2, 1500), (16, 5000), (3, 7500), (17, 900_000), (254, 750_000_000),
    ])
    yield "id3v2/test.rva2", b"Surround sound\0" + bytes((1, 0, 15, 4, 4, 3, 0, 21, 0, 8, 0, 30, 11, 255, 7))
    for encoding, name in [(0, "test.sylt"), (1, "test_utf16.sylt")]:
        def terminated(value):
            return utf16(value + "\0") if encoding else (value + "\0").encode("ascii")

        text = terminated("Test Sync Text")
        for ms, line in [(0, "Lofty"), (10000, "Is"), (15000, "Reading"), (30000, "This"), (1_938_000, "Correctly")]:
            text += terminated("\n" + line) + pack(">I", ms)
        yield f"id3v2/{name}", bytes([encoding]) + b"eng\x02\x01" + text

    track_atom = atom(b"trkn", (0, bytes((0, 0, 0, 1, 0, 0, 0, 0))))
    disc_atom = atom(b"disk", (0, bytes((0, 0, 0, 1, 0, 2))))
    yield "ilst/test.ilst", track_atom + disc_atom + b"".join(atom(key, (1, value)) for key, value in [
        (b"\xa9ART", artist), (b"\xa9alb", album), (b"\xa9cmt", comment), (b"\xa9day", year), (b"\xa9gen", genre), (b"\xa9nam", title),
    ])
    artist_atom = atom(b"\xa9ART", (1, b"Foo artist"))
    yield "ilst/issue_34.ilst", artist_atom + atom(b"plID", (21, pack(">Q", 88888)))
    yield "ilst/advisory_rating.ilst", artist_atom + atom(b"rtng", (21, b"\x04"))
    yield "ilst/multi_value_atom.ilst", atom(b"\xa9ART", (1, b"Foo artist"), (1, artist)) + atom(b"\xa9gen", (1, genre))
    yield "ilst/invalid_atom_type.ilst", track_atom + disc_atom + pack(">I4s", 8, b"\x01nam")
    yield "ilst/invalid_string_encoding.ilst", artist_atom + atom(b"\xa9alb", (2, b"\xd8\0")) + atom(b"\xa9nam", (1, b"Bar title")) + atom(b"\xa9cmt", (1, b"Baz comment"))
    yield "ilst/gnre_conversion_case_1.ilst", artist_atom + atom(b"gnre", (21, b"\0\x06"))
    yield "ilst/gnre_conversion_case_2.ilst", artist_atom + atom(b"gnre", (21, b"\0\x06")) + atom(b"\xa9gen", (1, b"My Custom Genre"))


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1] / "plain-rs/testdata/audio-tags"
    for name, data in fixtures():
        destination = root / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)

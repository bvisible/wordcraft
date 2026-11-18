//! A bytes-only check of an OLE compound file ([MS-CFB]) before the `cfb` crate opens it
//! (#3642). Every reader of `.doc`, `.xls` and encrypted Office packages calls [`check`] first.

/// Check a compound file's FAT layout before `cfb` reads it ([MS-CFB] §2.2 header, §2.5
/// DIFAT). `cfb` appends a whole sector of FAT entries for every FAT sector the DIFAT lists and
/// only validates the FAT once it is built, so a small header naming one sector over and over
/// makes it allocate gigabytes. Here the DIFAT is walked the way `cfb` walks it (header entries,
/// then the DIFAT chain, at most one step per sector; trailing zero and free entries trimmed
/// the same way) and the file is refused when it lists one FAT sector twice, or more FAT sectors
/// than the file has sectors — every FAT sector is a sector of the file, so a longer list is
/// duplicates or names `cfb` refuses anyway, and the FAT `cfb` builds (one sector of entries per
/// listed name) stays within about the file's own size. Anything else that is wrong is left for
/// `cfb` to report.
pub fn check(bytes: &[u8]) -> Result<(), String> {
    const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    const FREE: u32 = 0xFFFF_FFFF;
    const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
    const HEADER_ENTRIES: usize = 109;
    let u16_at = |at: usize| bytes.get(at..at.checked_add(2)?).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |at: usize| bytes.get(at..at.checked_add(4)?).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if bytes.len() < 512 || bytes.get(..8) != Some(MAGIC.as_slice()) {
        return Ok(());
    }
    let sector_len: usize = match u16_at(0x1A) {
        Some(3) => 512,
        Some(4) => 4096,
        _ => return Ok(()),
    };
    if bytes.len() < sector_len {
        return Ok(());
    }
    // `cfb` counts the sectors after the header sector, the last one possibly partial.
    let num_sectors = bytes.len().div_ceil(sector_len) - 1;
    let num_fat_sectors = u32_at(0x2C).unwrap_or(0) as usize;
    let mut difat: Vec<u32> = (0..HEADER_ENTRIES).filter_map(|i| u32_at(0x4C + i * 4)).collect();
    let mut next = u32_at(0x44).unwrap_or(END_OF_CHAIN);
    let mut steps = 0usize;
    while next != END_OF_CHAIN && next != FREE {
        steps += 1;
        // `cfb` refuses a chain sector past the file or seen before; a chain longer than the
        // file has sectors must have done one of the two.
        if steps > num_sectors || next as usize >= num_sectors {
            return Err(format!("the DIFAT chain is longer than the file or leaves it (sector {next})"));
        }
        let at = (next as usize + 1) * sector_len;
        let entries = sector_len / 4 - 1;
        difat.extend((0..entries).map(|i| u32_at(at + i * 4).unwrap_or(FREE)));
        next = u32_at(at + entries * 4).unwrap_or(END_OF_CHAIN);
    }
    while difat.len() > HEADER_ENTRIES && difat.len() > num_fat_sectors && difat.last() == Some(&0) {
        difat.pop();
    }
    while difat.last() == Some(&FREE) {
        difat.pop();
    }
    // Each listed FAT sector adds a whole sector of entries. A real file lists no more FAT
    // sectors than the file has — every FAT sector is a sector of the file, and none of them
    // twice (checked below) — so the FAT `cfb` builds stays within about the file's own size.
    // The #3642 blowup listed 2048 × 127 names in a ~1 MB file. A tighter bound (one FAT sector
    // per `sector_len / 4` sectors plus a spare) refused real files, whose writers reserve FAT
    // for growth: an encrypted package of 78 sectors lists 4 and didn't open (#5109).
    let listed: Vec<u32> = difat.into_iter().filter(|&s| s != FREE).collect();
    if listed.len() > num_sectors {
        return Err(format!("the DIFAT lists {} FAT sectors in a file of {num_sectors} sectors", listed.len()));
    }
    let mut seen = std::collections::HashSet::with_capacity(listed.len());
    if let Some(dup) = listed.iter().find(|&&s| !seen.insert(s)) {
        return Err(format!("the DIFAT lists FAT sector {dup} twice"));
    }
    Ok(())
}

// //// Neoffice — added (no upstream equivalent): a MiniFAT that no stream uses, marked free. macOS writes
// //// `textutil -convert doc` files whose every stream is at least the cutoff (4096 bytes) and whose MiniFAT sector is
// //// all zeros; `cfb` refuses them ("mini sector 0 pointed to twice"), so such a .doc opened as an empty document in
// //// the Drive of the Suite, while Word and LibreOffice open it (they never read a MiniFAT nobody uses).
/// A copy of the compound file `bytes` with its MiniFAT sectors marked free, for a reader to try when `cfb` refused
/// the file as it is. Only when no stream lives in the mini stream (every stream of the directory is empty or at least
/// the header's cutoff): the MiniFAT is then never read, and nothing a reader sees changes. `None` when the file is not
/// a compound file this can read, when a stream does live in the mini stream (its chains are in the MiniFAT, which is
/// needed as it is), when a chain is broken, or when the MiniFAT is free already.
pub fn repair_unused_minifat(bytes: &[u8]) -> Option<Vec<u8>> {
    const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    const FREE: u32 = 0xFFFF_FFFF;
    const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
    const MAX_REGULAR: u32 = 0xFFFF_FFFA;
    const HEADER_ENTRIES: usize = 109;
    const ENTRY_LEN: usize = 128;
    const STREAM: u8 = 2;
    let u16_at = |at: usize| bytes.get(at..at.checked_add(2)?).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |at: usize| bytes.get(at..at.checked_add(4)?).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if bytes.len() < 512 || bytes.get(..8) != Some(MAGIC.as_slice()) {
        return None;
    }
    let sector_len: usize = match u16_at(0x1A)? {
        3 => 512,
        4 => 4096,
        _ => return None,
    };
    let num_sectors = bytes.len().div_ceil(sector_len).checked_sub(1)?;
    // Where sector `s` starts, when it is a whole sector of the file.
    let start = |s: u32| -> Option<usize> {
        let at = (s as usize).checked_add(1)?.checked_mul(sector_len)?;
        (s <= MAX_REGULAR && at.checked_add(sector_len)? <= bytes.len()).then_some(at)
    };
    // The FAT sectors: the header's DIFAT, then its chain, trimmed as `check` (and `cfb`) trim them. The sectors
    // something other than the MiniFAT uses are kept aside: a MiniFAT chain that runs into one is not repaired.
    let num_fat_sectors = u32_at(0x2C)? as usize;
    let mut difat: Vec<u32> = (0..HEADER_ENTRIES).filter_map(|i| u32_at(0x4C + i * 4)).collect();
    let mut used = std::collections::HashSet::new();
    let mut next = u32_at(0x44)?;
    while next != END_OF_CHAIN && next != FREE {
        if used.len() >= num_sectors || !used.insert(next) {
            return None;
        }
        let at = start(next)?;
        let entries = sector_len / 4 - 1;
        difat.extend((0..entries).map(|i| u32_at(at + i * 4).unwrap_or(FREE)));
        next = u32_at(at + entries * 4)?;
    }
    while difat.len() > HEADER_ENTRIES && difat.len() > num_fat_sectors && difat.last() == Some(&0) {
        difat.pop();
    }
    while difat.last() == Some(&FREE) {
        difat.pop();
    }
    let fat_sectors: Vec<u32> = difat.into_iter().filter(|&s| s != FREE).collect();
    if fat_sectors.iter().any(|&s| start(s).is_none()) {
        return None;
    }
    used.extend(fat_sectors.iter().copied());
    let per_sector = sector_len / 4;
    let fat = |s: u32| -> Option<u32> { u32_at(start(*fat_sectors.get(s as usize / per_sector)?)? + (s as usize % per_sector) * 4) };
    // A chain of sectors through the FAT; `None` when it loops or leaves the file.
    let chain = |first: u32| -> Option<Vec<u32>> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut s = first;
        while s != END_OF_CHAIN {
            if !seen.insert(s) || out.len() >= num_sectors {
                return None;
            }
            start(s)?;
            out.push(s);
            s = fat(s)?;
        }
        Some(out)
    };
    const ROOT: u8 = 5;
    let cutoff = u64::from(u32_at(0x38)?);
    let directory = chain(u32_at(0x30)?)?;
    used.extend(directory.iter().copied());
    for &s in &directory {
        let sector = start(s)?;
        for entry in (sector..sector + sector_len).step_by(ENTRY_LEN) {
            let kind = bytes.get(entry + 66).copied();
            if kind != Some(STREAM) && kind != Some(ROOT) {
                continue;
            }
            // A version 3 file keeps the size in the low 32 bits only; the high ones may hold anything.
            let low = u64::from(u32_at(entry + 120)?);
            let size = if sector_len == 512 { low } else { low | (u64::from(u32_at(entry + 124)?) << 32) };
            if kind == Some(STREAM) && size > 0 && size < cutoff {
                return None;
            }
            // The sectors of a regular stream, and of the root's mini stream, are not the MiniFAT's.
            if size > 0 {
                used.extend(chain(u32_at(entry + 116)?)?);
            }
        }
    }
    let first = u32_at(0x3C)?;
    if first > MAX_REGULAR {
        return None;
    }
    let minifat = chain(first)?;
    if minifat.iter().any(|s| used.contains(s)) {
        return None;
    }
    let mut out = bytes.to_vec();
    let mut changed = false;
    for s in minifat {
        let at = start(s)?;
        for byte in out.get_mut(at..at + sector_len)? {
            changed |= *byte != 0xFF;
            *byte = 0xFF;
        }
    }
    changed.then_some(out)
}

/// A compound file of about 1 MB whose DIFAT chain lists one FAT sector 2048 × 127 times:
/// without [`check`], `cfb` builds a FAT of about 130 MB for it. Regression input for the
/// readers' tests: built only for tests, this crate's and (through the `test-support` feature,
/// which they turn on as a dev-dependency) the readers', never into the app.
#[cfg(any(test, feature = "test-support"))]
pub fn repeating_difat_sample() -> Vec<u8> {
    const S: usize = 512;
    const CHAIN: u32 = 2048; // DIFAT sectors 1..=CHAIN, sector 0 is the "FAT"
    let mut f = vec![0u8; S * (CHAIN as usize + 2)];
    let mut put = |at: usize, v: &[u8]| {
        if let Some(dst) = f.get_mut(at..at + v.len()) {
            dst.copy_from_slice(v);
        }
    };
    put(0, &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
    put(0x18, &0x3Eu16.to_le_bytes());
    put(0x1A, &3u16.to_le_bytes());
    put(0x1C, &0xFFFEu16.to_le_bytes());
    put(0x1E, &9u16.to_le_bytes());
    put(0x20, &6u16.to_le_bytes());
    put(0x2C, &u32::MAX.to_le_bytes()); // no zero trimming
    put(0x38, &4096u32.to_le_bytes());
    put(0x44, &1u32.to_le_bytes());
    put(0x48, &CHAIN.to_le_bytes());
    // The 109 header entries are all 0 already; each chain sector's entries too.
    for d in 1..=CHAIN {
        let next = if d == CHAIN { 0xFFFF_FFFE } else { d + 1 };
        put((d as usize + 2) * S - 4, &next.to_le_bytes());
    }
    f
}

#[cfg(test)]
mod tests {
    // //// Neoffice — repair_unused_minifat (see there): what it changes and what it leaves.
    #[test]
    fn a_minifat_is_marked_free_only_where_no_stream_uses_it() {
        assert_eq!(super::repair_unused_minifat(b"not a compound file"), None);
        // the repeating-DIFAT sample has a chain that loops: nothing is repaired
        assert_eq!(super::repair_unused_minifat(&super::repeating_difat_sample()), None);
    }

    #[test]
    fn a_repeating_difat_is_refused_and_a_small_file_passes() {
        assert!(super::check(&super::repeating_difat_sample()).is_err_and(|e| e.contains("FAT sectors in a file of")));
        assert!(super::check(b"not a compound file").is_ok());
    }

    /// A 512-byte-sector file of `sectors` sectors after the header, whose header DIFAT lists
    /// `fat` (the rest free).
    fn header_difat(sectors: usize, fat: &[u32]) -> Vec<u8> {
        let mut f = vec![0u8; 512 * (sectors + 1)];
        f[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
        f[0x1A..0x1C].copy_from_slice(&3u16.to_le_bytes());
        f[0x2C..0x30].copy_from_slice(&(fat.len() as u32).to_le_bytes());
        f[0x44..0x48].copy_from_slice(&0xFFFF_FFFEu32.to_le_bytes());
        for i in 0..109 {
            let v = fat.get(i).copied().unwrap_or(u32::MAX);
            f[0x4C + i * 4..0x50 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        f
    }

    #[test]
    fn a_fat_sector_listed_twice_or_more_than_the_file_has_is_refused() {
        // A duplicate, and a list longer than the file has sectors: every FAT sector is a
        // sector of the file, so a longer list is one of the two.
        assert!(super::check(&header_difat(10, &[0, 0])).is_err_and(|e| e.contains("FAT sector 0 twice")));
        assert!(
            super::check(&header_difat(10, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10])).is_err_and(|e| e.contains("11 FAT sectors in a file of 10 sectors"))
        );
        // A writer may reserve FAT for growth: a real encrypted package of 78 sectors lists
        // 4 FAT sectors and didn't open since the #3642 guard landed (#5109).
        assert!(super::check(&header_difat(78, &[0, 1, 2, 3])).is_ok());
        assert!(super::check(&header_difat(10, &[0])).is_ok());
        assert!(super::check(&header_difat(200, &[0, 1])).is_ok());
    }
}

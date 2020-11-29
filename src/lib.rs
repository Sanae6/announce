pub use crate::announcements::{Data, FreeWeekendState, HelloMessage, Language};
pub use crate::hazel::{HazelMessage, read_packet, write_packet};

pub mod announcements {
    use std::io::{Cursor, IoSliceMut, Read, Write};
    use std::io;
    use std::str::from_utf8;

    use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt, LittleEndian};

    use crate::hazel::{read_packed, write_packed};

    #[derive(Debug)]
    pub enum FreeWeekendState {
        NotFree,
        FreeMIRA,
        Free,
    }

    #[derive(Debug)]
    pub enum Data {
        CacheAnnouncement,
        Announcement((u32, String)),
        FreeWeekend(FreeWeekendState),
    }

    #[derive(Debug)]
    pub enum Language {
        English,
        Spanish,
        Portuguese,
        Korean,
        Russian,
    }

    impl From<Language> for u32{
        fn from(val: Language) -> u32 {
            match val {
                Language::English => 0,
                Language::Spanish => 1,
                Language::Portuguese => 2,
                Language::Korean => 3,
                Language::Russian => 4
            }
        }
    }

    impl From<u32> for Language{
        fn from(val: u32) -> Self {
            match val {
                0 => Language::English,
                1 => Language::Spanish,
                2 => Language::Portuguese,
                3 => Language::Korean,
                4 => Language::Russian,
                _ => Language::English
            }
        }
    }

    #[derive(Debug)]
    pub struct HelloMessage {
        pub version: u32,
        pub id: u32,
        pub language: Language,
    }

    pub fn read_hello(read: &mut dyn Read) -> io::Result<HelloMessage> {
        //read.read_u8()?;//hazel version
        read.read_u16::<BigEndian>()?;//unknown bytes
        let hello = HelloMessage {
            version: read_packed(read)?,
            id: read_packed(read)?,
            language: Language::from(read_packed(read)?),
        };
        Ok(hello)
    }

    pub fn write_hello(value: HelloMessage, write: &mut dyn Write) -> io::Result<()> {
        write.write_u16::<BigEndian>(0)?;
        write_packed(value.version, write)?;
        write_packed(value.id, write)?;
        write_packed(u32::from(value.language), write).map(|_|{})
    }

    pub fn read_data(read: &mut dyn Read) -> io::Result<Vec<Data>> {
        let mut datas = Vec::new();
        let mut tag = read.read_u8()?;
        loop {
            let _len = read.read_u16::<LittleEndian>()?;
            match tag {
                0 => datas.push(Data::CacheAnnouncement),
                1 => {
                    let id = read_packed(read)?;
                    let len = read_packed(read)?;
                    let mut vec: Vec<u8> = Vec::with_capacity(len as usize);
                    let len2 = read.read_vectored(&mut [IoSliceMut::new(vec.as_mut_slice())])?;
                    if len as usize != len2 {
                        return Err(io::Error::new(io::ErrorKind::InvalidData,
                                                  format!("Invalid string length: net = {} != read = {}", len, len2)));
                    }
                    let str = from_utf8(vec.as_slice()).map_err(|x| io::Error::new(io::ErrorKind::InvalidData,
                                                                                   format!("Invalid UTF-8 string: {:?}", x)))?;
                    datas.push(Data::Announcement((id, String::from(str))))
                }
                2 => {
                    match read.read_u8()? {
                        0 => datas.push(Data::FreeWeekend(FreeWeekendState::NotFree)),
                        1 => datas.push(Data::FreeWeekend(FreeWeekendState::FreeMIRA)),
                        2 => datas.push(Data::FreeWeekend(FreeWeekendState::Free)),
                        x => return Err(io::Error::new(io::ErrorKind::InvalidData,
                                                       format!("Invalid FreeWeekendState: {}", x)))
                    }
                }
                x => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Invalid data opcode: {}", x)));
                }
            }
            let res = read.read_u8();
            if let Err(_) = res {
                break;
            } else {
                tag = res?;
            }
        }
        Ok(datas)
    }

    pub fn write_data(value: Vec<Data>, write: &mut dyn Write) -> io::Result<()> {
        for val in value {
            match val {
                Data::CacheAnnouncement => {
                    write.write_u16::<LittleEndian>(0)?;
                    write.write_u8(0)?;
                }
                Data::Announcement((id, text)) => {
                    let buf = &mut Cursor::new(Vec::new());
                    write_packed(id, buf)?;
                    let textbuf = text.as_bytes();
                    write_packed(textbuf.len() as u32, buf)?;
                    buf.write(text.as_bytes())?;

                    write.write_u16::<LittleEndian>(buf.get_mut().len() as u16)?;
                    write.write_u8(1)?;
                    write.write(buf.get_mut().as_slice())?;
                }
                Data::FreeWeekend(w) => {
                    write.write_u16::<LittleEndian>(1)?;
                    write.write_u8(2)?;
                    write.write_u8(match w {
                        FreeWeekendState::NotFree => 0,
                        FreeWeekendState::FreeMIRA => 1,
                        FreeWeekendState::Free => 2
                    })?;
                }
            }
        }

        Ok(())
    }
}

pub mod hazel {
    use std::io::{Read, Write};
    use std::io;

    use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

    use crate::announcements::{Data, HelloMessage, read_data, read_hello, write_data, write_hello};

    #[derive(Debug)]
    pub enum HazelMessage {
        Unreliable(Vec<Data>),
        Reliable((u16, Vec<Data>)),
        Hello((u16, HelloMessage)),
        Disconnect,
        Ack(u16),
        Ping(u16),
    }

    pub fn read_packed(read: &mut dyn Read) -> io::Result<u32> {
        let mut read_more = true;
        let mut shift = 0;
        let mut output: u32 = 0;

        while read_more {
            let mut b = read.read_u8()? as u32;
            if b >= 0x80 {
                read_more = true;
                b ^= 0x80;
            } else {
                read_more = false;
            }

            output |= b << shift;
            shift += 7;
        }

        Ok(output)
    }

    pub fn write_packed(mut value: u32, write: &mut dyn Write) -> io::Result<usize> {
        if value == 0 {
            write.write_u8(0)?;
            return Ok(1);
        }
        let mut size = 0usize;
        while value > 0 {
            let mut b: u8 = (value & 0xff) as u8;
            if value >= 0x80 {
                b |= 0x80;
            }

            write.write_u8(b)?;
            size += 1;
            value >>= 7;
            if value == 0 {
                break;
            }
        }

        Ok(size)
    }

    pub fn read_packet(read: &mut dyn Read) -> io::Result<HazelMessage> {
        let op = read.read_u8()?;

        match op {
            0 => {
                Ok(HazelMessage::Unreliable(read_data(read)?))
            }
            1 => {
                Ok(HazelMessage::Reliable((read.read_u16::<BigEndian>()?, read_data(read)?)))
            }
            8 => {
                Ok(HazelMessage::Hello((read.read_u16::<BigEndian>()?, read_hello(read)?)))
            }
            9 => {
                Ok(HazelMessage::Disconnect)
            }
            10 => {
                Ok(HazelMessage::Ack(read.read_u16::<BigEndian>()?))
            }
            12 => {
                Ok(HazelMessage::Ping(read.read_u16::<BigEndian>()?))
            }
            x => {
                Err(io::Error::new(io::ErrorKind::InvalidData, format!("Invalid hazel opcode: {}", x)))
            }
        }
    }

    pub fn write_packet(value: HazelMessage, write: &mut dyn Write) -> io::Result<()> {
        match value {
            HazelMessage::Unreliable(val) => {
                write.write_u8(0)?;
                write_data(val, write)?
            }
            HazelMessage::Reliable((nonce, val)) => {
                write.write_u8(1)?;
                write.write_u16::<BigEndian>(nonce)?;
                write_data(val, write)?;
            }
            HazelMessage::Hello((nonce, hello)) => {
                write.write_u8(8)?;
                write.write_u16::<BigEndian>(nonce)?;
                write_hello(hello, write)?;
            }
            HazelMessage::Disconnect => {
                write.write_u8(9)?
            }
            HazelMessage::Ack(nonce) => {
                write.write_u8(10)?;
                write.write_u16::<BigEndian>(nonce)?;
            }
            HazelMessage::Ping(nonce) => {
                write.write_u8(12)?;
                write.write_u16::<BigEndian>(nonce)?;
            }
        }
        Ok(())
    }
}


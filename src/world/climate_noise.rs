// Климат 26.3: именованные потоки xoroshiro128++ и независимые фазы октав.
// https://minecraft.wiki/w/Random_sequence_format: «Xoroshiro128++» и «MD5».
// https://minecraft.wiki/w/Noise: «stable seeding»; модификатор октавы не
// должен сдвигать начало следующей. Перестановка и три начала принадлежат
// октаве, а не координате мира. Параметры проверяются переписью регионов;
// побитовую эквивалентность float32 в 26.3 [проверить].
struct Stream([u64; 2]);
impl Stream {
    fn seeded(seed: i64) -> Self {
        fn mix(mut v: u64) -> u64 {
            v = (v ^ (v >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            v = (v ^ (v >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            v ^ (v >> 31)
        }
        let lo = seed as u64 ^ 0x6A09_E667_F3BC_C909;
        Self([mix(lo), mix(lo.wrapping_add(0x9E37_79B9_7F4A_7C15))])
    }
    fn next(&mut self) -> u64 {
        let [a, mut b] = self.0;
        let out = a.wrapping_add(b).rotate_left(17).wrapping_add(a);
        b ^= a;
        self.0 = [a.rotate_left(49) ^ b ^ (b << 21), b.rotate_left(28)];
        out
    }
    fn fork(&mut self) -> Self { Self([self.next(), self.next()]) }
    fn named(&self, hash: [u64; 2]) -> Self { Self([self.0[0] ^ hash[0], self.0[1] ^ hash[1]]) }
    fn unit(&mut self) -> f64 { (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64) }
    fn below(&mut self, bound: u32) -> usize {
        loop {
            let product = (self.next() as u32 as u64) * u64::from(bound);
            if product as u32 >= bound.wrapping_neg() % bound { return (product >> 32) as usize; }
        }
    }
}
struct Octave { origin: [f64; 3], table: [u8; 256] }
impl Octave {
    fn new(mut rng: Stream) -> Self {
        let origin = std::array::from_fn(|_| rng.unit() * 256.0);
        let mut table = std::array::from_fn(|i| i as u8);
        for i in 0..256 { let j = i + rng.below((256-i) as u32); table.swap(i,j); }
        Self { origin, table }
    }
    fn at(&self, x: f64, y: f64, z: f64) -> f64 {
        let v = [x+self.origin[0],y+self.origin[1],z+self.origin[2]];
        let floor = v.map(f64::floor);
        let f: [f64; 3] = std::array::from_fn(|i| v[i]-floor[i]);
        let fade = f.map(|t|t*t*t*(t*(t*6.0-15.0)+10.0));
        let p = |i: i64| i64::from(self.table[(i & 255) as usize]);
        let mut sum=0.0;
        for dx in 0..2 { for dy in 0..2 { for dz in 0..2 {
            let h = p(p(p(floor[0] as i64+dx)+floor[1] as i64+dy)+floor[2] as i64+dz) & 15;
            let [x,y,z]=[f[0]-dx as f64,f[1]-dy as f64,f[2]-dz as f64];
            let u=if h<8 {x} else {y};
            let v=if h<4 {y} else if h==12 || h==14 {x} else {z};
            let g=(if h&1==0 {u} else {-u})+(if h&2==0 {v} else {-v});
            sum+=g*(if dx==0 {1.0-fade[0]} else {fade[0]})
                *(if dy==0 {1.0-fade[1]} else {fade[1]})*(if dz==0 {1.0-fade[2]} else {fade[2]});
        }}}
        sum
    }
}
struct Field { octaves: [Vec<(Octave,f64,f64)>;2], factor: f64 }
impl Field {
    fn new(seed: i64, id: usize, base: i32, amplitudes: &[f64]) -> Self {
        let mut random=Stream::seeded(seed).fork().named(NAMES[id]);
        let octaves=std::array::from_fn(|_| {
            let positional=random.fork();
            amplitudes.iter().enumerate().filter(|(_,a)| **a!=0.0).map(|(i,&a)| {
                let n=base+i as i32;
                (Octave::new(positional.named(OCTAVES[(n+12) as usize])), 2f64.powi(n),
                 a*2f64.powi((amplitudes.len()-i-1) as i32)/(2f64.powi(amplitudes.len() as i32)-1.0))
            }).collect()
        });
        let first=amplitudes.iter().position(|&a|a!=0.0).unwrap();
        let last=amplitudes.iter().rposition(|&a|a!=0.0).unwrap();
        Self {octaves, factor: (5.0/3.0)/(1.0+1.0/(last-first+1) as f64)}
    }
    fn at(&self,x:f64,y:f64,z:f64)->f64 {
        let mut total=0.0;
        for (i,octaves) in self.octaves.iter().enumerate() {
            let s=if i==0 {1.0} else {1.018_126_888_217_522_7};
            for (o,f,a) in octaves {total+=o.at(x*s*f,y*s*f,z*s*f)*a;}
        }
        total*self.factor
    }
}
// MD5 идентификаторов: порядок temperature, vegetation, continentalness,
// erosion, ridge, offset; половины читаются big-endian.
const NAMES: [[u64;2];6] = [
    [0x5C7E6B29735F0D7F,0xF7D86F1BBC734988],
    [0x81BB4D22E8DC168E,0xF1C8B4BEA16303CD],
    [0x83886C9D0AE3A662,0xAFA638A61B42E8AD],
    [0xD02491E6058F6FD8,0x4792512C94C17A80],
    [0xEFC8EF4D36102B34,0x1BEEEB324A0F24EA],
    [0x080518CF6AF25384,0x3F3DFB40A54FEBD5],
];
// MD5 строк octave_-12 … octave_0, без пространства имён.
const OCTAVES: [[u64;2];13] = [
    [0xB198DE63A8012672,0x7B84CAD43EF7B5A8],
    [0x0FD787BFBC403EC3,0x74A4A31CA21B48B8],
    [0x36D326EED40EFEB2,0x5BE9CE18223C636A],
    [0x082FE255F8BE6631,0x4E96119E22DEDC81],
    [0x0EF68EC68504005E,0x48B6BF93A2789640],
    [0xF11268128982754F,0x257A1D670430B0AA],
    [0xE51C98CE7D1DE664,0x5F9478A733040C45],
    [0x6D7B49E7E429850A,0x2E3063C622A24777],
    [0xBD90D5377BA1B762,0xC07317D419A7548D],
    [0x53D39C6752DAC858,0xBCD1C5A80AB65B3E],
    [0xB4A24D7A84E7677B,0x023FF9668E89B5C4],
    [0xDFFA22B534C5F608,0xB9B67517D3665CA9],
    [0xD50708086CEF4D7C,0x6E1651ECC7F43309],
];

pub(super) struct ClimateFields { fields: [Field; 6] }
impl ClimateFields {
    pub(super) fn new(seed: i64) -> Self {
        Self { fields: [
            Field::new(seed, 0, -10, &[1.5,0.,1.,0.,0.,0.]),
            Field::new(seed, 1, -8, &[1.,1.,0.,0.,0.,0.]),
            Field::new(seed, 2, -9, &[1.,1.,2.,2.,2.,1.,1.,1.,1.]),
            Field::new(seed, 3, -9, &[1.,1.,0.,1.,1.]),
            Field::new(seed, 4, -7, &[1.,2.,1.,0.,0.,0.]),
            Field::new(seed, 5, -3, &[1.,1.,1.,0.]),
        ] }
    }
    // Сдвиг прибавляется после масштаба 1/4. Прежний порядок делил сам
    // сдвиг ещё на четыре и терял мелкие краевые пятна пустыни.
    pub(super) fn at(&self, x: i32, z: i32) -> super::Climate {
        let (x,z)=(f64::from(x & !3),f64::from(z & !3));
        let shift=&self.fields[5];
        let dx=shift.at(x*0.25,0.,z*0.25)*4.;
        let dz=shift.at(z*0.25,x*0.25,0.)*4.;
        let (x,z)=(x*0.25+dx,z*0.25+dz);
        super::Climate {
            temperature: self.fields[0].at(x,0.,z),
            humidity: self.fields[1].at(x,0.,z),
            continent: self.fields[2].at(x,0.,z),
            erosion: self.fields[3].at(x,0.,z),
            weirdness: self.fields[4].at(x,0.,z),
        }
    }
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    #[test]
    fn climate_salts_are_md5_of_names_not_numeric_seed_xors() {
        use md5::{Digest, Md5};
        let check = |name: &str, expected: [u64;2]| {
            let digest = Md5::digest(name.as_bytes());
            assert_eq!(expected, [u64::from_be_bytes(digest[..8].try_into().unwrap()),
                u64::from_be_bytes(digest[8..].try_into().unwrap())]);
        };
        for (i,name) in ["temperature","vegetation","continentalness","erosion","ridge","offset"].iter().enumerate() {
            check(&format!("minecraft:{name}"),NAMES[i]);
        }
        for (i, hash) in OCTAVES.iter().enumerate() { check(&format!("octave_{}",i as i32-12),*hash); }
    }

    #[test]
    fn enabling_an_octave_does_not_reseed_its_neighbours() {
        let a = Field::new(42, 0, -8, &[1.0,0.0,1.0]);
        let b = Field::new(42, 0, -8, &[1.0,1.0,1.0]);
        for pair in 0..2 { for (left,right) in [(0,0),(1,2)] {
            let (a,b)=(&a.octaves[pair][left].0,&b.octaves[pair][right].0);
            assert_eq!(a.origin,b.origin);
            assert_eq!(a.table,b.table);
        } }
        assert_ne!(a.octaves[0][0].0.origin,a.octaves[1][0].0.origin);
    }

    #[test]
    fn seed_phase_is_not_a_shared_origin_zero() {
        let values: Vec<_> = [0,1,42,-1,i64::MIN,i64::MAX].into_iter()
            .map(|seed| ClimateFields::new(seed).at(0,0).continent).collect();
        assert!(values.iter().all(|v| v.is_finite() && v.abs()>1e-6));
        for (i,a) in values.iter().enumerate() { assert!(values[i+1..].iter().all(|b| (a-b).abs()>1e-6)); }
    }
}

// Узкие извилистые ходы — отдельный проход после шумовых полостей.
// Вики, Cave: «carver caves are added later», «irregular tunnels»;
// Monster Room: «always nearby carver caves».
// https://minecraft.wiki/w/Cave https://minecraft.wiki/w/Monster_Room
// Форма своя [проверить]: сверяется статистика комнат, не совпадение пещер.
// Все точки зависят только от семени и чанка-источника; каждый получатель
// рисует пересечение, поэтому ни порядок чанков, ни их загрузка не важны.
use super::*;

const SALT: u64 = 0xCA_71_01;
const STEPS: usize = 32;
const STEP: f64 = 2.0;

fn path(key: u64, cx: i32, cz: i32) -> Vec<(f64,f64,f64,f64)> {
    let mut random = Random::new(hash(key,cx,0,cz,SALT) as i64);
    if next_unit(&mut random) >= 0.25 { return Vec::new(); }
    let (mut x,mut z) = (f64::from(cx*16+next_in(&mut random,0,15)),f64::from(cz*16+next_in(&mut random,0,15)));
    // Вики: начало от -56 до 180, чаще ниже 47. Верхний хвост убывает.
    // Отрицательные высоты выровнены по измерениям 26.1.2: комнаты
    // в полосах -48..-33 / -32..-17 / -16..-1 встречаются 230/220/226 раз.
    // Частота ходов 1/4 — калибровка своей геометрии, не правило vanilla;
    // число попыток комнат остаётся 10+4 [проверить на других семенах].
    let upper = next_in(&mut random,0,236);
    let y = -56+next_in(&mut random,0,upper);
    let mut y = f64::from(if y < 0 { next_in(&mut random,-56,-1) } else { y });
    let mut angle = next_unit(&mut random)*std::f64::consts::TAU;
    let mut pitch = (next_unit(&mut random)-0.5)*0.12;
    let radius = 1.4+next_unit(&mut random)*0.8;
    let mut points = Vec::with_capacity(STEPS);
    for i in 0..STEPS {
        let taper = (i.min(STEPS-1-i) as f64 / 4.0).min(1.0);
        points.push((x,y,z,radius*(0.7+0.3*taper)));
        x += angle.cos()*STEP;
        z += angle.sin()*STEP;
        y += pitch;
        angle += (next_unit(&mut random)-0.5)*0.3;
        pitch = pitch*0.8+(next_unit(&mut random)-0.5)*0.16;
    }
    points
}

pub(super) fn carve(chunk: &mut Chunk, cx: i32, cz: i32, key: u64, palette: &Palette) {
    let (bx,bz) = (cx*16,cz*16);
    // 64 блока пути и радиус <3: источники не дальше пяти чанков.
    for sx in cx-5..=cx+5 { for sz in cz-5..=cz+5 {
        for (x,y,z,r) in path(key,sx,sz) {
            if x+r < f64::from(bx) || x-r > f64::from(bx+16) || z+r < f64::from(bz) || z-r > f64::from(bz+16) { continue; }
            for wx in ((x-r).floor() as i32).max(bx)..=((x+r).ceil() as i32).min(bx+15) {
                for wz in ((z-r).floor() as i32).max(bz)..=((z+r).ceil() as i32).min(bz+15) {
                    let horizontal = ((f64::from(wx)+0.5-x).powi(2)+(f64::from(wz)+0.5-z).powi(2))/(r*r);
                    for wy in ((y-1.5).floor() as i32).max(-54)..=((y+1.5).ceil() as i32).min(TOP-1) {
                        if horizontal+((f64::from(wy)+0.5-y)/1.5).powi(2) >= 1.0 { continue; }
                        let here = chunk.block(wx-bx,wy,wz-bz);
                        if here == palette.bedrock || palette.liquid(here) || here == AIR { continue; }
                        // Жидкие блоки остаются; сопряжение с водоносными
                        // полостями [проверить].
                        chunk.put_generated(wx-bx,wy,wz-bz,AIR,true);
                    }
                }
            }
        }
    } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carving_is_repeatable_across_negative_chunk_edges() {
        let palette = Palette::new();
        let make = |cx,cz| {
            let mut chunk = Chunk::new();
            for x in 0..16 { for z in 0..16 { for y in -54..64 { chunk.put_generated(x,y,z,palette.stone,true); } } }
            carve(&mut chunk,cx,cz,1234,&palette);
            chunk
        };
        let west = make(-1,0);
        let east = make(0,0);
        let east_again = make(0,0);
        let west_again = make(-1,0);
        for x in 0..16 { for z in 0..16 { for y in -54..64 {
            assert_eq!(west.block(x,y,z),west_again.block(x,y,z));
            assert_eq!(east.block(x,y,z),east_again.block(x,y,z));
        } } }
        let first = (-10..10).flat_map(|x|(-10..10).map(move |z|(x,z))).find(|&(x,z)|!path(1234,x,z).is_empty()).unwrap();
        assert_eq!(path(1234,first.0,first.1),path(1234,first.0,first.1));
        assert_ne!(path(1234,first.0,first.1),path(1235,first.0,first.1));
        for (x,y,z,r) in path(1234,first.0,first.1) {
            assert!(x >= f64::from(first.0*16)-64.0 && x <= f64::from(first.0*16+16)+64.0);
            assert!(z >= f64::from(first.1*16)-64.0 && z <= f64::from(first.1*16+16)+64.0);
            assert!((-60.0..185.0).contains(&y)); assert!(r < 3.0);
        }
    }
}

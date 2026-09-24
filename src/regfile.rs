//! Banque de registres RISC-V (32 registres x0..x31).
//!
//! À toi d'écrire l'implémentation en dessous des tests pour les faire
//! passer. Pas encore besoin du trait `RegisterFile` de lib-rv32 (ça viendra
//! à l'étape 2 du TODO, une fois `cpu/` vendoré) : pour l'instant, une API
//! "maison" suffit.
//!
//! Ce qu'il faut construire (voir les tests ci-dessous pour la signature
//! exacte attendue) :
//! - un type `RegisterFile` avec un constructeur `new()` qui démarre avec
//!   tous les registres à 0 ;
//! - une méthode `read(&self, num: u8) -> u32` ;
//! - une méthode `write(&mut self, num: u8, data: u32)` ;
//! - x0 doit **toujours** lire 0, même après une écriture dessus (contrainte
//!   RISC-V : x0 est câblé à zéro dans le vrai hardware).
pub struct RegisterFile {
    registers: [u32; 32],
}

impl RegisterFile {
    pub fn new() -> Self {
        Self {
            registers: [0u32; 32],
        }
    }

    pub fn read(&self, num: u8) -> u32 {
        self.registers[num as usize]
    }

    pub fn write(&mut self, num: u8, data: u32) {
        if num == 0 {
        } else {
            self.registers[num as usize] = data
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nouveaux_registres_sont_a_zero() {
        let rf = RegisterFile::new();
        for i in 0..32u8 {
            assert_eq!(rf.read(i), 0, "x{i} devrait valoir 0 au départ");
        }
    }

    #[test]
    fn ecriture_puis_lecture_registre_normal() {
        let mut rf = RegisterFile::new();
        rf.write(5, 42);
        assert_eq!(rf.read(5), 42);
    }

    #[test]
    fn x0_reste_toujours_zero() {
        let mut rf = RegisterFile::new();
        rf.write(0, 0xDEADBEEF);
        assert_eq!(
            rf.read(0),
            0,
            "x0 doit rester câblé à zéro même après écriture"
        );
    }

    #[test]
    fn les_registres_sont_independants() {
        let mut rf = RegisterFile::new();
        rf.write(1, 111);
        rf.write(2, 222);
        assert_eq!(rf.read(1), 111);
        assert_eq!(rf.read(2), 222);
        // écrire dans x2 ne doit pas affecter x1
        rf.write(2, 999);
        assert_eq!(rf.read(1), 111);
    }
}

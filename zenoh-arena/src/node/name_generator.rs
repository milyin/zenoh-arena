/// Name generation utilities for creating human-readable node names
use markov_namegen::{CharacterChainGenerator, RandomTextGenerator};

/// Fantasy names for training data - mix of various cultures and fantasy settings
const TRAINING_NAMES: &[&str] = &[
    // Famous dragons from literature and media
    "Smaug", "Ancalagon", "Glaurung", "Saphira", "Eragon", "Thorn", "Firnen", "Glaedr",
    "Drogon", "Rhaegal", "Viserion", "Balerion", "Vhagar", "Meraxes", "Sunfyre", "Meleys",
    "Caraxes", "Syrax", "Vermithrax", "Draco", "Falkor", "Toothless", "Hookfang", "Stormfly",
    "Meatlug", "Barf", "Belch", "Alduin", "Paarthurnax", "Seath", "Kalameet", "Midir",
    "Spyro", "Cynder", "Fafnir", "Nidhoggr", "Jormungandr", "Tiamat", "Bahamut", "Melanchthon", 
    // Asian dragons
    "Shenlong", "Ryujin", "Mizuchi", "Kuraokami", "Watatsumi", "Zhulong", "Yinglong",
    "Tianlong", "Fucanglong", "Dilong", "Panlong", "Jiaolong", "Qiulong",
    // Mythology dragons
    "Ladon", "Python", "Hydra", "Typhon", "Vritra", "Apalala", "Leviathan", "Quetzalcoatl",
    "Apep", "Yamata", "Orochi", "Feilong", "Xiuhcoatl", "Zilant", "Zmey", "Gorynych",
    // European legendary dragons
    "Knucker", "Wyvern", "Lindworm", "Tatzelwurm", "Peluda", "Tarasque", "Gargouille",
    "Cuelebre", "Guivre", "Bolla", "Kulshedra", "Balaur", "Zilant", "Zmaj", "Smok",
    // Fantasy and games
    "Alexstrasza", "Deathwing", "Ysera", "Malygos", "Nozdormu", "Neltharion",
    "Ridley", "Grima", "Naga", "Dovahkiin", "Akatosh", "Parthurnax",
];

/// Create a new name generator
fn create_name_generator() -> CharacterChainGenerator {
    CharacterChainGenerator::builder()
        .with_order(2)  // Use bigrams for smoother names
        .with_prior(0.01)  // Some randomness
        .train(TRAINING_NAMES.iter().copied())
        .build()
}

/// Generate a human-readable random name
/// 
/// Uses Markov chain-based name generation to create pronounceable,
/// fantasy-style names that are easy to remember and distinguish.
/// The names are validated to ensure they contain only alphanumeric
/// characters and underscores (keyexpr-compatible).
/// 
/// # Examples
/// - "Theron"
/// - "Aldric"
/// - "Mirabel"
/// - "Gareth"
pub fn generate_random_name() -> String {
    let mut generator = create_name_generator();
    
    // Generate until we get a valid name
    loop {
        let name = generator.generate_one();
        
        // Validate that the name only contains alphanumeric characters
        // and is a reasonable length
        if !name.is_empty() 
            && name.len() <= 12  // Keep names reasonably short
            && name.chars().all(|c| c.is_alphanumeric() || c == '_') 
        {
            return name;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_name() {
        let name = generate_random_name();
        assert!(!name.is_empty());
        assert!(name.len() <= 12);
        assert!(name.chars().all(|c| c.is_alphanumeric() || c == '_'));
    }

    #[test]
    fn test_names_are_different() {
        // Generate multiple names and ensure they're not all the same
        let names: Vec<String> = (0..1000).map(|_| generate_random_name()).collect();
        let unique_count = names.iter().collect::<std::collections::HashSet<_>>().len();
        assert!(unique_count > 5, "Should generate reasonably unique names");
    }
}

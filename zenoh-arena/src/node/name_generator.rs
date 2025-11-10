/// Name generation utilities for creating human-readable node names
use markov_namegen::{CharacterChainGenerator, RandomTextGenerator};

/// Fantasy names for training data - mix of various cultures and fantasy settings
const TRAINING_NAMES: &[&str] = &[
    // Fantasy inspired
    "Aldric", "Theron", "Gareth", "Mirabel", "Isolde", "Lysander", "Elowen", "Rowan",
    "Caelum", "Astrid", "Eirik", "Freya", "Magnus", "Sigrid", "Bjorn", "Ingrid",
    // Greek/Roman inspired
    "Apollo", "Diana", "Atlas", "Selene", "Orion", "Luna", "Phoenix", "Aurora",
    // Celtic inspired
    "Finn", "Maeve", "Cormac", "Niamh", "Declan", "Siobhan", "Aidan", "Brigid",
    // Nordic inspired
    "Ragnar", "Skald", "Torsten", "Astrid", "Gunnar", "Helga", "Ivar", "Sigrun",
    // More fantasy
    "Zephyr", "Ember", "Storm", "Raven", "Wolf", "Bear", "Hawk", "Fox",
    "Cedar", "Ash", "Oak", "Birch", "Willow", "Maple", "Pine", "Elm",
];

/// Create a new name generator
fn create_name_generator() -> CharacterChainGenerator {
    CharacterChainGenerator::builder()
        .with_order(2)  // Use bigrams for smoother names
        .with_prior(0.01)  // Some randomness
        .train(TRAINING_NAMES.iter().map(|s| *s))
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

/// Generate a random name with a numeric suffix for uniqueness
/// 
/// Creates a name like "Theron_42" or "Aldric_123" to ensure uniqueness
/// while maintaining human readability.
pub fn generate_unique_name() -> String {
    let base_name = generate_random_name();
    let suffix: u16 = rand::random::<u16>() % 1000;
    format!("{}_{}", base_name, suffix)
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
    fn test_generate_unique_name() {
        let name = generate_unique_name();
        assert!(!name.is_empty());
        assert!(name.contains('_'));
        
        // Verify format: name_number
        let parts: Vec<&str> = name.split('_').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].chars().all(|c| c.is_alphanumeric()));
        assert!(parts[1].chars().all(|c| c.is_numeric()));
    }

    #[test]
    fn test_names_are_different() {
        // Generate multiple names and ensure they're not all the same
        let names: Vec<String> = (0..10).map(|_| generate_unique_name()).collect();
        let unique_count = names.iter().collect::<std::collections::HashSet<_>>().len();
        assert!(unique_count > 5, "Should generate reasonably unique names");
    }
}

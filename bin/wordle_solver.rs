use eloquentle::{
    filter::{Filter, get_best_first_guess},
    game::WordleGame,
    ranking::{HybridStrategy, LetterFrequencyStrategy, RankingStrategy},
};
use std::collections::HashSet;
use std::time::Instant;

fn main() {
    // Create a new Wordle filter with the default word list
    let filter = Filter::default();
    println!("Starting with {} possible words", filter.remaining_count());

    // First guess using precomputed value - should be instant
    let start_time = Instant::now();
    let first_guess = get_best_first_guess();
    let duration = start_time.elapsed();
    println!(
        "Precomputed best first guess: {} (calculated in {:?})",
        first_guess, duration
    );

    // Compare with calculating the first guess - might be slow
    let start_time = Instant::now();
    let first_guess_calculated = filter.recommend_guess_from_candidates(true);
    let duration = start_time.elapsed();
    println!(
        "Calculated first guess: {} (calculated in {:?})",
        first_guess_calculated, duration
    );

    println!("\n=== Example 1: Manual feedback with performance measurement ===");
    // Simulate a game where we manually apply feedback
    // Let's say our first guess is "arise" and we get the following feedback:
    // 'a': yellow (right letter, wrong position)
    // 'r': yellow (right letter, wrong position)
    // 'i': gray (not in word)
    // 's': yellow (right letter, wrong position)
    // 'e': yellow (right letter, wrong position)

    let mut filter1 = Filter::default();
    // Apply filters based on this feedback
    filter1.filter_contains('a');
    filter1.filter_contains('r');
    filter1.filter_without('i');
    filter1.filter_contains('s');
    filter1.filter_contains('e');
    filter1.filter_not_at('a', 0);
    filter1.filter_not_at('r', 1);
    filter1.filter_not_at('s', 3);
    filter1.filter_not_at('e', 4);

    // Print remaining candidates
    println!(
        "After first guess, {} possible words remain",
        filter1.remaining_count()
    );

    // Get next recommendation with timing
    if filter1.remaining_count() > 1 {
        let start_time = Instant::now();
        let next_guess = filter1.recommend_guess();
        let duration = start_time.elapsed();
        println!(
            "Next recommended guess: {} (calculated in {:?})",
            next_guess, duration
        );
    }

    println!("\n=== Example 2: Automated simulation with performance measurement ===");
    // Create a new game and simulate guesses automatically
    let mut filter2 = Filter::default();
    let target_word = "rates";
    println!("Target word: {}", target_word);

    // Create a game instance
    let game = WordleGame::new(target_word.to_string());

    // First guess
    let start_time = Instant::now();
    let guess1 = "arise"; // Using a fixed first guess
    let _duration = start_time.elapsed();
    println!("First guess: {} (instant because it's hardcoded)", guess1);

    // Get feedback from the game
    let info_set1 = game.get_feedback(guess1);
    filter2.add_info_set(&info_set1);

    println!(
        "Feedback: {:?} ({} words remain)",
        info_set1,
        filter2.remaining_count()
    );

    // Second guess with timing
    if filter2.remaining_count() > 1 {
        let start_time = Instant::now();
        let guess2 = filter2.recommend_guess();
        let duration = start_time.elapsed();
        println!("Second guess: {} (calculated in {:?})", guess2, duration);

        // Get feedback from the game
        let info_set2 = game.get_feedback(&guess2);
        filter2.add_info_set(&info_set2);

        println!(
            "Feedback: {:?} ({} words remain)",
            info_set2,
            filter2.remaining_count()
        );

        // Continue until we find the word or run out of guesses
        if filter2.remaining_count() > 1 {
            let start_time = Instant::now();
            let guess3 = filter2.recommend_guess();
            let duration = start_time.elapsed();
            println!("Third guess: {} (calculated in {:?})", guess3, duration);

            // Get feedback from the game
            let info_set3 = game.get_feedback(&guess3);
            filter2.add_info_set(&info_set3);

            println!(
                "Feedback: {:?} ({} words remain)",
                info_set3,
                filter2.remaining_count()
            );
        }
    }

    if filter2.remaining_count() == 1 {
        println!("Found the word: {}", filter2.remaining_words()[0]);
    }

    // Add performance test with decreasing candidate sets
    println!("\n=== Performance Test with Decreasing Candidate Sets ===");
    let mut perf_filter = Filter::default();

    // Create a hybrid ranking strategy
    let hybrid_strategy = HybridStrategy::default();
    let letter_freq_strategy = LetterFrequencyStrategy;

    // Measure performance with full dictionary (using precomputed value)
    let start_time = Instant::now();
    let _ = perf_filter.recommend_guess();
    let duration = start_time.elapsed();
    println!(
        "Recommendation with {} candidates: {:?}",
        perf_filter.remaining_count(),
        duration
    );

    // Reduce to ~1000 words
    perf_filter.filter_contains('a');
    let start_time = Instant::now();
    let _ = perf_filter.recommend_guess();
    let duration = start_time.elapsed();
    println!(
        "Recommendation with {} candidates: {:?}",
        perf_filter.remaining_count(),
        duration
    );

    // Reduce further to ~100 words
    perf_filter.filter_contains('e');
    let start_time = Instant::now();
    let _ = perf_filter.recommend_guess();
    let duration = start_time.elapsed();
    println!(
        "Recommendation with {} candidates: {:?}",
        perf_filter.remaining_count(),
        duration
    );

    // Reduce to very few words
    perf_filter.filter_contains('r');
    let start_time = Instant::now();
    let _ = perf_filter.recommend_guess();
    let duration = start_time.elapsed();
    println!(
        "Recommendation with {} candidates: {:?}",
        perf_filter.remaining_count(),
        duration
    );

    // Demonstrate the new ranking strategies
    println!("\n=== Ranking Strategy Comparison ===");
    let sample_filter = Filter::default();
    let candidates = &sample_filter.remaining_words()[0..10]; // Just use first 10 for display

    println!("Sample of remaining words: {:?}", candidates);

    // Use letter frequency strategy for a quick recommendation
    let start_time = Instant::now();
    let letter_freq_guess = letter_freq_strategy.rank_words(
        &sample_filter.get_candidates(),
        &candidates.iter().map(|s| s.as_str()).collect::<Vec<&str>>(),
    )[0];
    let duration = start_time.elapsed();

    println!(
        "Letter frequency strategy recommendation: {} (in {:?})",
        letter_freq_guess, duration
    );

    // Use hybrid strategy
    let start_time = Instant::now();
    let hybrid_guess = hybrid_strategy.rank_words(
        &sample_filter.get_candidates(),
        &candidates.iter().map(|s| s.as_str()).collect::<Vec<&str>>(),
    )[0];
    let duration = start_time.elapsed();

    println!(
        "Hybrid strategy recommendation: {} (in {:?})",
        hybrid_guess, duration
    );

    // Demonstrate the new Info system
    println!("\n=== Info System Demonstration ===");
    let mut info_filter = Filter::default();

    // Add some information manually
    info_filter.add_info(eloquentle::Info::Correct('s', 0));
    info_filter.add_info(eloquentle::Info::NotAt('a', 1));
    info_filter.add_info(eloquentle::Info::Not('e'));

    println!(
        "After adding info: {} words remain",
        info_filter.remaining_count()
    );

    // Show the collected info
    println!("Collected info: {:?}", info_filter.get_info());
}

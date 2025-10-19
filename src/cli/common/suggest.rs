use strsim::damerau_levenshtein;

const SIMILARITY_THRESHOLD: usize = 8;

pub fn get_closest<'a, I, S>(input: &str, choices: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a S>,
    S: AsRef<str> + 'a,
{
    let mut best_score = usize::MAX;
    let mut best_match: Option<&'a str> = None;

    for choice in choices {
        let choice_str = choice.as_ref();
        let dist = damerau_levenshtein(&input.to_lowercase(), &choice_str.to_lowercase());

        if dist < best_score {
            best_score = dist;
            best_match = Some(choice_str);
        }
    }

    println!("{best_score}");
    if best_score <= SIMILARITY_THRESHOLD {
        best_match
    } else {
        None
    }
}

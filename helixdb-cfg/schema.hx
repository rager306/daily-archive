N::Paper {
    INDEX arxiv_id: String,
    title: String,
    published: String,
    score: F32
}

N::Author {
    INDEX name: String
}

N::Keyword {
    INDEX word: String
}

N::Category {
    INDEX name: String
}

E::authored_by {
    From: Paper,
    To: Author,
    Properties: {
    }
}

E::tagged_with {
    From: Paper,
    To: Keyword,
    Properties: {
    }
}

E::belongs_to {
    From: Paper,
    To: Category,
    Properties: {
    }
}

V::AbstractEmbedding {
    paper_id: String
}


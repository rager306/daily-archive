from pathlib import Path
import ladybug
import logging

logger = logging.getLogger(__name__)

DB_DIR = Path.home() / ".research" / "graph_db"

def init_db(db_path: Path | str = DB_DIR) -> ladybug.Connection:
    """Initialize LadybugDB and ensure the graph schema exists.
    
    Creates:
    - Node tables: Paper, Author, Keyword, Category
    - Rel tables: AUTHORED_BY, TAGGED_WITH, BELONGS_TO
    - Installs and loads the 'algo' extension for graph math.
    """
    path_str = str(db_path)
    # Ensure directory parent exists
    Path(path_str).parent.mkdir(parents=True, exist_ok=True)
    
    db = ladybug.Database(path_str)
    conn = ladybug.Connection(db)
    
    # Install & Load extensions
    try:
        conn.execute("INSTALL algo;")
        conn.execute("LOAD EXTENSION algo;")
    except Exception as e:
        logger.warning(f"Could not load algo extension: {e}")

    try:
        # Schema definition. We use explicit transactions for DDL? DDL is auto-commit in Ladybug.
        # Paper node with 1024-dim embedding for deepvk/USER-bge-m3
        conn.execute("CREATE NODE TABLE Paper(id STRING, title STRING, published DATE, emb FLOAT[512], score DOUBLE, PRIMARY KEY (id))")
        conn.execute("CREATE NODE TABLE Author(name STRING, PRIMARY KEY (name))")
        conn.execute("CREATE NODE TABLE Keyword(word STRING, PRIMARY KEY (word))")
        conn.execute("CREATE NODE TABLE Category(name STRING, PRIMARY KEY (name))")
        
        conn.execute("CREATE REL TABLE AUTHORED_BY(FROM Paper TO Author)")
        conn.execute("CREATE REL TABLE TAGGED_WITH(FROM Paper TO Keyword)")
        conn.execute("CREATE REL TABLE BELONGS_TO(FROM Paper TO Category)")
        
        logger.info("LadybugDB schema created successfully.")
    except RuntimeError as e:
        if "already exists" in str(e).lower():
            logger.info("LadybugDB schema already exists.")
        else:
            raise

    return conn

def upsert_daily_analysis(conn: ladybug.Connection, analysis: "DailyAnalysis") -> None:
    """Bulk upsert a DailyAnalysis payload into LadybugDB.
    
    Uses explicit transactions and parameterized MERGE statements to handle deduplication 
    gracefully and ensure atomic single-writer concurrency.
    """
    if analysis.status == "empty" or not analysis.papers:
        return
        
    conn.execute("BEGIN TRANSACTION")
    try:
        for p in analysis.papers:
            paper = p.paper
            
            # 1. Upsert Paper
            emb_list = p.embedding if p.embedding else []
            conn.execute(
                "MERGE (p:Paper {id: $id}) "
                "ON MATCH SET p.title = $title, p.published = date($published), p.emb = $emb, p.score = $score "
                "ON CREATE SET p.title = $title, p.published = date($published), p.emb = $emb, p.score = $score",
                {
                    "id": paper.id, 
                    "title": paper.title, 
                    "published": paper.published.isoformat(), 
                    "emb": emb_list, 
                    "score": p.score
                }
            )
            
            # 2. Upsert Authors and AUTHORED_BY
            for author in paper.authors:
                conn.execute("MERGE (a:Author {name: $name})", {"name": author})
                conn.execute(
                    "MATCH (p:Paper {id: $id}), (a:Author {name: $name}) "
                    "MERGE (p)-[:AUTHORED_BY]->(a)",
                    {"id": paper.id, "name": author}
                )
                
            # 3. Upsert Categories and BELONGS_TO
            for cat in paper.categories:
                conn.execute("MERGE (c:Category {name: $name})", {"name": cat})
                conn.execute(
                    "MATCH (p:Paper {id: $id}), (c:Category {name: $name}) "
                    "MERGE (p)-[:BELONGS_TO]->(c)",
                    {"id": paper.id, "name": cat}
                )
                
            # 4. Upsert Keywords and TAGGED_WITH
            for keyword in p.keywords:
                conn.execute("MERGE (k:Keyword {word: $word})", {"word": keyword})
                conn.execute(
                    "MATCH (p:Paper {id: $id}), (k:Keyword {word: $word}) "
                    "MERGE (p)-[:TAGGED_WITH]->(k)",
                    {"id": paper.id, "word": keyword}
                )
                
        conn.execute("COMMIT")
        logger.info(f"Bulk upserted {len(analysis.papers)} papers into LadybugDB.")
    except Exception as e:
        conn.execute("ROLLBACK")
        logger.error(f"Failed to bulk upsert papers: {e}")
        raise

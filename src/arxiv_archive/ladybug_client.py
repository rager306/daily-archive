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

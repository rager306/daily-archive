import os
from pathlib import Path
import helix
from helix.schema import Schema

def init_db(config_path: str = "helixdb-cfg") -> Schema:
    """Initialize HelixDB schema for arXiv papers.
    
    Creates nodes for Paper, Keyword, Author, and Category.
    Creates edges representing relationships.
    Creates vector mapping for Paper abstracts.
    """
    schema = Schema(config_path=config_path)
    
    # 1. Nodes
    # Paper node
    schema.create_node(
        "Paper",
        properties={
            "arxiv_id": "String",
            "title": "String",
            "published": "String",
            "score": "F32",
        },
        index=["arxiv_id"]
    )
    
    # Author node
    schema.create_node(
        "Author",
        properties={"name": "String"},
        index=["name"]
    )
    
    # Keyword node
    schema.create_node(
        "Keyword",
        properties={"word": "String"},
        index=["word"]
    )
    
    # Category node
    schema.create_node(
        "Category",
        properties={"name": "String"},
        index=["name"]
    )
    
    # 2. Edges
    # Paper -> Author
    schema.create_edge(
        "authored_by",
        from_node="Paper",
        to_node="Author",
        properties={}
    )
    
    # Paper -> Keyword
    schema.create_edge(
        "tagged_with",
        from_node="Paper",
        to_node="Keyword",
        properties={}
    )
    
    # Paper -> Category
    schema.create_edge(
        "belongs_to",
        from_node="Paper",
        to_node="Category",
        properties={}
    )
    
    # 3. Vectors
    # Paper abstract embeddings (Assuming 384 dim for sentence-transformers like all-MiniLM-L6-v2)
    # Helix handles vectors using V::VectorType
    schema.create_vector(
        "AbstractEmbedding",
        properties={
            "paper_id": "String"
        }
    )
    
    schema.save()
    return schema

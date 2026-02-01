// @generated automatically by Diesel CLI.

pub mod api {
    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        api.answers (id) {
            id -> Uuid,
            question_id -> Uuid,
            body -> Text,
            submitted_by -> Uuid,
            upvotes -> Int4,
            downvotes -> Int4,
            created_at -> Timestamptz,
            updated_at -> Timestamptz,
        }
    }

    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        api.api_keys (id) {
            id -> Uuid,
            user_id -> Uuid,
            #[max_length = 64]
            key_hash -> Varchar,
            #[max_length = 16]
            key_prefix -> Varchar,
            #[max_length = 100]
            name -> Varchar,
            last_used -> Nullable<Timestamptz>,
            expires_at -> Nullable<Timestamptz>,
            created_at -> Timestamptz,
        }
    }

    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        api.question_tags (question_id, tag_id) {
            question_id -> Uuid,
            tag_id -> Uuid,
        }
    }

    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        api.questions (id) {
            id -> Uuid,
            title -> Text,
            body -> Text,
            embedding -> Vector,
            submitted_by -> Uuid,
            created_at -> Timestamptz,
            updated_at -> Timestamptz,
        }
    }

    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        api.tag_synonyms (id) {
            id -> Uuid,
            #[max_length = 35]
            synonym -> Varchar,
            tag_id -> Uuid,
            created_at -> Timestamptz,
            updated_at -> Timestamptz,
        }
    }

    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        api.tags (id) {
            id -> Uuid,
            #[max_length = 35]
            name -> Varchar,
            created_at -> Timestamptz,
            updated_at -> Timestamptz,
        }
    }

    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        api.users (id) {
            id -> Uuid,
            #[max_length = 255]
            workos_id -> Varchar,
            github_id -> Nullable<Int8>,
            #[max_length = 39]
            username -> Varchar,
            created_at -> Timestamptz,
            updated_at -> Timestamptz,
        }
    }

    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        api.votes (id) {
            id -> Uuid,
            answer_id -> Uuid,
            user_id -> Uuid,
            value -> Int4,
            created_at -> Timestamptz,
        }
    }

    diesel::joinable!(answers -> questions (question_id));
    diesel::joinable!(answers -> users (submitted_by));
    diesel::joinable!(api_keys -> users (user_id));
    diesel::joinable!(question_tags -> questions (question_id));
    diesel::joinable!(question_tags -> tags (tag_id));
    diesel::joinable!(questions -> users (submitted_by));
    diesel::joinable!(tag_synonyms -> tags (tag_id));
    diesel::joinable!(votes -> answers (answer_id));
    diesel::joinable!(votes -> users (user_id));

    diesel::allow_tables_to_appear_in_same_query!(
        answers,api_keys,question_tags,questions,tag_synonyms,tags,users,votes,);
}

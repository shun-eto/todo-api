create table labels 
(
  id serial primary key,
  name text not null
);

create table todo_labels 
(
  id serial primary key,
  todo_id integer not null references todos (id) deferrable initially deferred,
  label_id integer not null references labels (id) deferrable initially deferred
);
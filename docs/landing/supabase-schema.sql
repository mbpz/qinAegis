-- Supabase Analytics Schema for qinAegis Landing Page
-- Run this in Supabase SQL Editor

-- Enable UUID extension
create extension if not exists "uuid-ossp";

-- Page views table
create table if not exists page_views (
  id uuid default uuid_generate_v4() primary key,
  visited_at timestamptz default now(),
  page_path text not null,
  referrer text,
  user_agent text,
  screen_width int,
  screen_height int,
  country text,
  city text
);

-- Enable Row Level Security
alter table page_views enable row level security;

-- Allow public inserts (tracking from landing page)
create policy "Allow public inserts" on page_views
  for insert with check (true);

-- Analytics summary function
create or replace function get_page_stats(days int default 7)
returns table (
  date date,
  views bigint,
  unique_visitors bigint
) as $$
  select 
    date(visited_at) as date,
    count(*) as views,
    count(distinct left(user_agent, 50)) as unique_visitors
  from page_views
  where visited_at > now() - interval '1 day' * days
  group by date(visited_at)
  order by date desc;
$$ language sql security definer;

-- Track page views with geo info (optional enhancement)
create or replace function log_page_view(
  p_page_path text,
  p_referrer text default null,
  p_user_agent text default null,
  p_screen_width int default null,
  p_screen_height int default null
) returns void as $$
  insert into page_views (page_path, referrer, user_agent, screen_width, screen_height)
  values (p_page_path, p_referrer, p_user_agent, p_screen_width, p_screen_height);
$$ language sql security definer;
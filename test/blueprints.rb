require 'faker'

Sham.name  { Faker::Name.name }
Sham.email { Faker::Internet.email }
Sham.title { Faker::Lorem.sentence }
Sham.body  { Faker::Lorem.paragraph }
Sham.url   { Faker::Internet.domain_name }

Event.blueprint do
  title { Sham.title }
  description { Sham.body }
  starts_at { Time.now + 1.week }
  ends_at { Time.now + 1.week + 1.hour }
  url {Sham.url}
  calendar (Calendar.make)
end

Calendar.blueprint do
  title { Sham.title }
  description { Sham.body }
  url {Sham.url}
end
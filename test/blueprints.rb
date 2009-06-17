require 'faker'

Sham.name  { Faker::Name.name }
Sham.email { Faker::Internet.email }
Sham.title { Faker::Lorem.sentence }
Sham.body  { Faker::Lorem.paragraph }
Sham.url   { Faker::Internet.domain_name }

Event.blueprint do
  title { Sham.title }
  description { Sham.body }
  start_epoch { (Time.now + 1.week).to_i }
  end_epoch { (Time.now + 1.week + 1.hour).to_i }
  url {Sham.url}
end

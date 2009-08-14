class CreateLocations < ActiveRecord::Migration
  def self.up
    create_table :locations do |t|
      t.integer 'geoname_id'
      t.string  'fcodeName'
      t.string  'adminCode1'
      t.string  'fcl'
      t.string  'admin_name'
      t.float   'longitude'
      t.float   'latitude'
      t.string  'country_name'
      t.integer 'population'
      t.string  'country_code'
      t.string  'fcode'
      t.string  'fclName'
      t.timestamps
    end
  end

  def self.down
    drop_table :locations
  end
end


